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
    pub trojan: Trojan,
    pub tls: Tls,
    pub redis: Option<Redis>,
}

fn default_listen() -> String {
    String::from("0.0.0.0:443")
}

#[derive(Deserialize, Debug)]
pub struct Trojan {
    #[serde(default = "default_listen")]
    pub listen: String,
    #[serde(default)]
    pub password: Vec<String>,
    pub fallback: String,
}

#[derive(Deserialize, Debug)]
pub struct Tls {
    pub sni: String,
    pub cert: String,
    pub key: String,
}

fn default_redis() -> String {
    String::from("127.0.0.1:6379")
}

#[derive(Deserialize, Debug)]
pub struct Redis {
    #[serde(default = "default_redis")]
    pub server: String,
}

pub fn load_config_from_path(s: &str) -> Result<Config> {
    let config_string = std::fs::read_to_string(s)?;
    let config = toml::from_str(&config_string)?;

    Ok(config)
}
