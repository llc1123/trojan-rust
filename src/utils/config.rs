use anyhow::Result;
use serde_derive::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Server,
}

impl Default for Mode {
    fn default() -> Self {
        Mode::Server
    }
}

#[derive(Deserialize, Debug)]
pub struct Config {
    #[serde(default)]
    pub mode: Mode,
    #[serde(default)]
    pub trojan: Trojan,
    pub tls: Tls,
    pub redis: Option<Redis>,
}

#[derive(Deserialize, Debug, Default)]
pub struct Trojan {
    #[serde(default)]
    pub password: Vec<String>,
    #[serde(default)]
    pub fallback: String,
}

fn default_listen() -> String {
    String::from("0.0.0.0:443")
}

#[derive(Deserialize, Debug)]
pub struct Tls {
    #[serde(default = "default_listen")]
    pub listen: String,
    pub sni: String,
    pub cert: String,
    pub key: String,
}

fn default_redis() -> String {
    String::from("127.0.0.1:6379")
}

#[derive(Deserialize, Debug, Clone)]
pub struct Redis {
    #[serde(default = "default_redis")]
    pub server: String,
}

pub fn load(s: &str) -> Result<Config> {
    let config_string = std::fs::read_to_string(s)?;
    let config = toml::from_str(&config_string)?;

    Ok(config)
}
