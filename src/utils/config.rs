pub use self::client::Config as ClientConfig;
pub use self::server::Config as ServerConfig;
use anyhow::Result;
use serde_derive::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(tag = "mode")]
#[serde(rename_all = "lowercase")]
pub enum Config {
    Server(ServerConfig),
    Client(ClientConfig),
}

pub fn load(s: &str) -> Result<Config> {
    let config_string = std::fs::read_to_string(s)?;
    let config = toml::from_str(&config_string)?;

    Ok(config)
}

pub(crate) mod client {
    use serde_derive::Deserialize;

    fn default_bind() -> String {
        String::from("127.0.0.1:1080")
    }

    #[derive(Deserialize, Debug)]
    pub struct TrojanServer {
        /// hostname:port
        pub server: String,
        /// password in plain text
        pub password: String,

        /// enable udp or not
        #[serde(default)]
        pub udp: bool,

        /// sni
        pub sni: String,
        /// skip certificate verify
        #[serde(default)]
        pub skip_cert_verify: bool,
    }

    #[derive(Deserialize, Debug)]
    pub struct Config {
        #[serde(default = "default_bind")]
        pub bind: String,
        #[serde(default)]
        pub tcp_nodelay: bool,

        pub server: TrojanServer,
    }
}

pub(crate) mod server {
    use serde_derive::Deserialize;
    use serde_with::{formats::PreferMany, serde_as, OneOrMany};

    #[derive(Deserialize, Debug)]
    pub struct Config {
        #[serde(default)]
        pub trojan: Trojan,
        pub tls: Tls,
        #[serde(default)]
        pub outbound: Outbound,
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

    #[serde_as]
    #[derive(Deserialize, Debug)]
    pub struct Tls {
        #[serde(default = "default_listen")]
        pub listen: String,
        #[serde(default)]
        pub tcp_nodelay: bool,
        #[serde(default)]
        #[serde_as(deserialize_as = "OneOrMany<_, PreferMany>")]
        pub sni: Vec<String>,
        pub cert: String,
        pub key: String,
    }

    #[derive(Deserialize, Debug, Default)]
    pub struct Outbound {
        #[serde(default)]
        pub block_local: bool,
    }

    fn default_redis() -> String {
        String::from("127.0.0.1:6379")
    }

    #[derive(Deserialize, Debug, Clone)]
    pub struct Redis {
        #[serde(default = "default_redis")]
        pub server: String,
    }
}
