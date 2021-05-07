use clap::{AppSettings, Clap};
use service::Config;
mod service;
use anyhow::Result;
use chrono;
use log::error;

fn setup_logger(&log_level: &log::LevelFilter) -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.level(),
                message
            ))
        })
        .level(log_level)
        .chain(std::io::stdout())
        .apply()?;
    Ok(())
}

#[derive(Clap)]
#[clap(version = "1.0", author = "llc1123 <i@llc.moe>")]
#[clap(setting = AppSettings::ColoredHelp)]
struct Opts {
    #[clap(short, long, default_value = "config.toml")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opts::parse();
    let config_string = std::fs::read_to_string(opts.config)?;
    let config: Config = toml::from_str(&config_string)?;

    setup_logger(&config.log_level).unwrap();

    if let Err(e) = service::start(config) {
        error!("Unable to start service: {}", e)
    }
    Ok(())
}
