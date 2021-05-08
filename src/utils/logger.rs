use anyhow::Result;
use fern::colors::{Color, ColoredLevelConfig};
use log::{info, LevelFilter};
use std::str::FromStr;

pub fn setup_logger(log_level: &str) -> Result<()> {
    let loglevel = LevelFilter::from_str(log_level).unwrap_or_else(|err| {
        eprintln!("Error parsing log_level: {}", err);
        LevelFilter::Info
    });

    let colors = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        .trace(Color::BrightBlack);

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{}[{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                colors.color(record.level()),
                message
            ))
        })
        .level(loglevel)
        .chain(std::io::stdout())
        .apply()?;
    info!("log_level={}", loglevel);
    Ok(())
}
