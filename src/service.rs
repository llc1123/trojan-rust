use log::trace;
use log::LevelFilter;
use serde_derive::Deserialize;
use serde_with::{serde_as, DisplayFromStr};

#[derive(Deserialize, Debug)]
pub enum Mode {
    Server,
}

impl Default for Mode {
    fn default() -> Self {
        Mode::Server
    }
}

#[serde_as]
#[derive(Deserialize, Debug)]
pub struct Config {
    #[serde(default)]
    pub mode: Mode,
    #[serde(default = "default_level")]
    #[serde_as(as = "DisplayFromStr")]
    pub log_level: LevelFilter,
    pub trojan: Trojan,
    pub tls: Tls,
    pub redis: Option<Redis>,
}

fn default_level() -> LevelFilter {
    LevelFilter::Info
}

#[derive(Deserialize, Debug)]
pub struct Trojan {
    pub listen: Option<String>,
    pub password: Option<String>,
    pub fallback: String,
}

#[derive(Deserialize, Debug)]
pub struct Tls {
    pub sni: String,
    pub cert: String,
    pub key: String,
}

#[derive(Deserialize, Debug)]
pub struct Redis {
    pub server: String,
}

pub fn start(config: Config) -> Result<i8, &'static str> {
    trace!("{:?}", &config);
    Err("this is a test message.")
}
