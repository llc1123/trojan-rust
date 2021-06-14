#![feature(ip)]

mod auth;
mod common;
mod inbound;
mod outbound;
mod relay;
mod server;
mod utils;

use anyhow::{Context, Result};
use clap::{AppSettings, Clap};
use log::error;
use utils::{config, logger};

#[derive(Clap)]
#[clap(version, author, about)]
#[clap(setting = AppSettings::ColoredHelp)]
struct Opts {
    #[clap(short, long, default_value = "config.toml")]
    config: String,
    #[clap(long, default_value = "info", env = "LOGLEVEL")]
    log_level: String,
}

async fn start(config: &str) -> Result<()> {
    let config = config::load(config).context("Failed to parse config")?;
    server::start(config)
        .await
        .context("Failed to start service")?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opts::parse();
    logger::setup_logger(opts.log_level.as_str())?;

    if let Err(e) = start(opts.config.as_str()).await {
        error!("{:?}", e);
    }

    Ok(())
}
