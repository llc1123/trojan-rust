use crate::{
    auth::AuthHub,
    inbound::{
        fallback::FallbackAcceptor,
        tls_openssl,
        trojan::{self, TrojanAcceptor},
    },
    outbound::direct,
    utils::{config::Config, pushing_stream::PushingStream},
};
use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use openssl::ssl::SslAcceptor;
use std::{net::SocketAddr, sync::Arc};
use tokio::net::{TcpListener, TcpStream};
use tokio_openssl::SslStream;

struct ConnectionConfig {
    tls_acceptor: SslAcceptor,
    sni: String,
    fallback_acceptor: FallbackAcceptor,
    trojan_acceptor: TrojanAcceptor,
}

impl ConnectionConfig {
    async fn accept(&self, stream: TcpStream, peer_addr: SocketAddr) -> Result<()> {
        info!("Inbound connection from {}", peer_addr);
        let stream = self.tls_acceptor.accept(stream).await?;
        let (_, session) = stream.get_ref();
        debug!(
            "ALPN: {:?}",
            session.get_alpn_protocol().unwrap_or_default()
        );
        debug!("SNI: {:?}", session.get_sni_hostname().unwrap_or_default());
        let sni_matched = session
            .get_sni_hostname()
            .map(|x| x == self.sni)
            .unwrap_or(false);
        let stream = PushingStream::new(stream);

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
    // let tls_acceptor = tls::from(&config.tls).context("Failed to setup TLS server.")?;
    let tls_acceptor = tls_openssl::new(&config.tls)?;
    let fallback_acceptor = FallbackAcceptor::new(config.trojan.fallback)
        .await
        .context("Failed to setup fallback server.")?;

    let trojan_acceptor = trojan::TrojanAcceptor::new(auth_hub)?;

    let conn_cfg = Arc::new(ConnectionConfig {
        tls_acceptor,
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
