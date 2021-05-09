use crate::inbound::tls;
use crate::utils::config::Config;
use anyhow::Result;
use log::{debug, info};
use std::sync::Arc;
use tokio::io::{copy, sink, AsyncWriteExt};
use tokio_rustls::rustls::Session;

pub async fn start(config: Config) -> Result<()> {
    debug!("Loading Config: {:?}", &config);
    let inbound = tls::TlsInbound::new(&config.tls).await?;
    info!("Service started.");
    let tls_config = Arc::new(config.tls);
    loop {
        let tls_config = tls_config.clone();
        let (stream, peer_addr) = inbound.tcp_listener.accept().await?;
        let acceptor = inbound.tls_acceptor.clone();
        let fut = async move {
            info!("Inbound connection from {}", peer_addr);
            let mut output = sink();
            let mut stream = acceptor.accept(stream).await?;
            let (_, session) = stream.get_ref();
            debug!(
                "ALPN: {:?}",
                session.get_alpn_protocol().unwrap_or_default()
            );
            debug!("SNI: {:?}", session.get_sni_hostname().unwrap_or_default());
            // TODO: redirect to fallback on SNI mismatch
            match session.get_sni_hostname() {
                Some(x) if x == tls_config.sni => (),
                _ => (),
            }
            stream
                .write_all(
                    &b"HTTP/1.0 200 ok\r\n\
                    Connection: close\r\n\
                    Content-length: 12\r\n\
                    \r\n\
                    Hello world!"[..],
                )
                .await?;
            stream.shutdown().await?;
            copy(&mut stream, &mut output).await?;
            info!("Hello: {}", peer_addr);

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
