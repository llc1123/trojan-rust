use crate::inbound;
use crate::utils::config::Config;
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

    let fallback_inbound = inbound::fallback::from(inbound::fallback::Config {})?;

    info!("Service started.");

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let tls_acceptor = tls_inbound.clone();
        let tls_config = tls_config.clone();

        let fallback_acceptor = fallback_inbound.clone();

        let fut = async move {
            info!("Inbound connection from {}", peer_addr);
            let stream = tls_acceptor.accept(stream).await?;
            let (_, session) = stream.get_ref();
            debug!(
                "ALPN: {:?}",
                session.get_alpn_protocol().unwrap_or_default()
            );
            debug!("SNI: {:?}", session.get_sni_hostname().unwrap_or_default());

            match session.get_sni_hostname() {
                Some(x) if x == tls_config.sni => {
                    info!("SNI match.");
                    // trojan_acceptor.accept(stream).await?
                }
                _ => {
                    info!("SNI mismatch. Redirect to fallback.");
                    fallback_acceptor.accept(stream).await?
                }
            };

            Ok(()) as Result<()>
        };

        tokio::spawn(async move {
            if let Err(err) = fut.await {
                eprintln!("{:?}", err);
            }
        });
    }

    info!("Service stopped.");
    Ok(())
}
