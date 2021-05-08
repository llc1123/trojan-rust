mod service;
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

fn start(config: &str) -> Result<()> {
    let config = config::load_config_from_path(config).context("Failed to parse config")?;
    service::start(config).context("Failed to start service")?;
    Ok(())
}

fn main() -> Result<()> {
    let opts = Opts::parse();
    logger::setup_logger(opts.log_level.as_str())?;

    if let Err(e) = start(opts.config.as_str()) {
        error!("{:?}", e);
    }

    Ok(())
}
