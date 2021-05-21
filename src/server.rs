use crate::{
    auth::AuthHub,
    inbound::{fallback::FallbackAcceptor, tls, trojan::TrojanAcceptor},
    outbound::direct,
    utils::config::Config,
};
use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use openssl::ssl::{NameType, Ssl, SslContext};
use std::{net::SocketAddr, pin::Pin, sync::Arc};
use tokio::net::{TcpListener, TcpStream};
use tokio_openssl::SslStream;

struct ConnectionConfig {
    ssl_context: SslContext,
    sni: String,
    fallback_acceptor: FallbackAcceptor,
    trojan_acceptor: TrojanAcceptor,
}

impl ConnectionConfig {
    async fn accept(&self, stream: TcpStream, peer_addr: SocketAddr) -> Result<()> {
        info!("Inbound connection from {}", peer_addr);
        let mut stream = SslStream::new(Ssl::new(&self.ssl_context)?, stream)?;
        Pin::new(&mut stream)
            .accept()
            .await
            .context("Invalid TLS connection.")?;
        let servername = stream
            .ssl()
            .servername(NameType::HOST_NAME)
            .unwrap_or_default();
        debug!("SNI: {:?}", &servername);
        let sni_matched = servername == self.sni;

        if sni_matched {
            match self.trojan_acceptor.accept(stream).await {
                Ok(out) => direct::accept(out).await?,
                Err(stream) => {
                    self.fallback_acceptor.accept(stream).await?;
                }
            };
        } else {
            warn!("Redirect to fallback: SNI mismatch.");
            self.fallback_acceptor.accept(stream).await?;
        };

        Ok(())
    }
}

pub async fn start(config: Config) -> Result<()> {
    debug!("Loading Config: {:?}", &config);

    let listener = TcpListener::bind(&config.tls.listen.as_str())
        .await
        .context(format!("Failed to bind address {}", &config.tls.listen))?;

    let auth_hub = AuthHub::new(&config).await?;
    let ssl_context = tls::TlsContext::new(&config.tls).context("Failed to setup TLS server.")?;
    let fallback_acceptor = FallbackAcceptor::new(config.trojan.fallback)
        .await
        .context("Failed to setup fallback server.")?;

    let trojan_acceptor = TrojanAcceptor::new(auth_hub)?;

    let conn_cfg = Arc::new(ConnectionConfig {
        ssl_context: ssl_context.inner,
        sni: config.tls.sni,
        fallback_acceptor,
        trojan_acceptor,
    });

    info!("Service started.");

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        stream
            .set_nodelay(config.tls.tcp_nodelay)
            .context("Set TCP_NODELAY failed")?;
        let conn_cfg = conn_cfg.clone();
        tokio::spawn(async move {
            if let Err(err) = conn_cfg.accept(stream, peer_addr).await {
                error!("{:?}", err);
            }
        });
    }

    // info!("Service stopped.");
    // Ok(())
}
