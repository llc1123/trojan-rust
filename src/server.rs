use std::{sync::Arc, time::Duration};

use crate::{
    auth::{Auth, AuthHub},
    inbound::{tls::TlsContext, trojan::TrojanInbound},
    outbound::direct::DirectOutbound,
    relay::Relay,
    utils::config::Config,
};
use anyhow::{Context, Result};
use log::{debug, info};
use tokio::net::TcpListener;

pub async fn start(config: Config) -> Result<()> {
    debug!("Loading Config: {:?}", &config);

    let listener = TcpListener::bind(&config.tls.listen.as_str())
        .await
        .context(format!("Failed to bind address {}", &config.tls.listen))?;

    let auth_hub: Arc<dyn Auth> = Arc::new(AuthHub::new(&config).await?);
    let tls_context = TlsContext::new(&config.tls).context("Failed to setup TLS server.")?;
    let inbound = TrojanInbound::new(auth_hub.clone(), tls_context, config.trojan).await?;
    let outbound = DirectOutbound::new();

    let mut relay = Relay::new(listener, inbound, outbound, config.tls.tcp_nodelay);
    relay.tcp_timeout = Some(Duration::from_secs(600));

    info!("Service started.");

    relay.serve_trojan(auth_hub).await?;

    info!("Service stopped.");
    Ok(())
}
