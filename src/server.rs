use crate::{
    auth::AuthHub,
    inbound::{
        fallback::FallbackAcceptor,
        tls,
        trojan::{self, TrojanAcceptor},
    },
    utils::{config::Config, peekable_stream::PeekableStream},
};
use anyhow::{Context, Result};
use log::{debug, error, info};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{rustls::Session, TlsAcceptor};
use trojan::Cmd;

struct ConnectionConfig {
    tls_acceptor: TlsAcceptor,
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

        let mut stream = PeekableStream::new(stream);

        if sni_matched {
            match self.trojan_acceptor.accept(&mut stream).await {
                Ok(Cmd::Connect) => {
                    todo!("Connect")
                }
                Ok(Cmd::UdpAssociate) => {
                    todo!("Udp")
                }
                Err(e) => {
                    debug!("Trojan accept error: {:?}. Redirect to fallback.", e);
                    self.fallback_acceptor.accept(stream).await?;
                }
            };
        } else {
            info!("SNI mismatch. Redirect to fallback.");
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
    let tls_acceptor = tls::from(&config.tls).context("Failed to setup TLS server.")?;
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
