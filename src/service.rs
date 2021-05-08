use crate::utils::config::Config;
use anyhow::Result;
use log::{error, info, trace};

pub fn start(config: Config) -> Result<()> {
    trace!("Config: {:?}", &config);
    info!("Service started.");
    error!("this is a test message.");

    Ok(())
}
