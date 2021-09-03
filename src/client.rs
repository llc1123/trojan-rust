use std::time::Duration;

use crate::{
    inbound::socks5::Socks5Inbound, outbound::trojan::TrojanOutbound, relay::Relay,
    utils::config::ClientConfig,
};
use anyhow::{Context, Result};
use log::{debug, info};
use tokio::net::TcpListener;

pub async fn start(config: ClientConfig) -> Result<()> {
    debug!("Loading Config: {:?}", &config);

    let listener = TcpListener::bind(&config.bind)
        .await
        .context(format!("Failed to bind address {}", &config.bind))?;

    let inbound = Socks5Inbound::new();
    let outbound = TrojanOutbound::new(config.server)?;

    let mut relay = Relay::new(listener, inbound, outbound, config.tcp_nodelay);
    relay.tcp_timeout = Some(Duration::from_secs(600));

    info!("Service started.");

    relay.serve_socks5().await?;

    Ok(())
}
