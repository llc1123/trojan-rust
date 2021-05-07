use log::trace;
use serde_derive::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub mode: Option<String>,
    pub log_level: Option<String>,
    pub trojan: Trojan,
    pub tls: Tls,
    pub redis: Option<Redis>,
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
