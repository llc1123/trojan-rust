use crate::utils::config::Config;
use crate::{inbound, utils::peekable_stream::PeekableStream};
use anyhow::{Context, Result};
use log::{debug, info};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_rustls::rustls::Session;

pub async fn start(config: Config) -> Result<()> {
    debug!("Loading Config: {:?}", &config);

    let listener = TcpListener::bind(config.tls.listen.as_str())
        .await
        .context(format!("Failed to bind address {}", config.tls.listen))?;

    let tls_inbound = inbound::tls::from(&config.tls)?;
    let tls_config = Arc::new(config.tls);

    let fallback_inbound = inbound::fallback::FallbackAcceptor::new(inbound::fallback::Config {
        target: config.trojan.fallback,
    })?;
    let trojan_acceptor = inbound::trojan::TrojanAcceptor::new()?;

    info!("Service started.");

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let tls_acceptor = tls_inbound.clone();
        let tls_config = tls_config.clone();

        let fallback_acceptor = fallback_inbound.clone();
        let trojan_acceptor = trojan_acceptor.clone();

        let fut = async move {
            info!("Inbound connection from {}", peer_addr);
            let stream = tls_acceptor.accept(stream).await?;
            let (_, session) = stream.get_ref();
            debug!(
                "ALPN: {:?}",
                session.get_alpn_protocol().unwrap_or_default()
            );
            debug!("SNI: {:?}", session.get_sni_hostname().unwrap_or_default());
            let sni_matched = session
                .get_sni_hostname()
                .map(|x| x == tls_config.sni)
                .unwrap_or(false);

            let mut stream = PeekableStream::new(stream);

            if sni_matched {
                info!("SNI match.");
                match trojan_acceptor.accept(&mut stream).await {
                    Ok(()) => {}
                    Err(e) => {
                        debug!("Trojan accept error: {:?}. Redirect to fallback.", e);
                        fallback_acceptor.accept(stream).await?;
                    }
                };
            } else {
                info!("SNI mismatch. Redirect to fallback.");
                fallback_acceptor.accept(stream).await?;
            };

            Ok(()) as Result<()>
        };

        tokio::spawn(async move {
            if let Err(err) = fut.await {
                eprintln!("{:?}", err);
            }
        });
    }

    // info!("Service stopped.");
    // Ok(())
}
