pub mod from_config;
pub mod redis;

use self::{from_config::ConfigAuthenticator, redis::RedisAuthenticator};
use crate::utils::config::Config;
use anyhow::{anyhow, Result};
use async_trait::async_trait;

#[async_trait]
pub trait Auth {
    async fn auth(&self, password: String) -> Result<bool>;
    async fn stat(&self, password: String, upload: u64, download: u64) -> Result<()>;
}

pub struct AuthHub {
    config_auth: ConfigAuthenticator,
    redis_auth: Option<RedisAuthenticator>,
}

impl AuthHub {
    pub async fn new(config: &Config) -> Result<AuthHub> {
        let config_auth = ConfigAuthenticator::new(config.trojan.password)?;
        match config.redis {
            Some(redis) => Ok(AuthHub {
                config_auth,
                redis_auth: Some(RedisAuthenticator::new(redis.server)?),
            }),
            None => Ok(AuthHub {
                config_auth,
                redis_auth: None,
            }),
        }
    }
}

#[async_trait]
impl Auth for AuthHub {
    async fn auth(&self, password: String) -> Result<bool> {
        if self.config_auth.auth(password).await? {
            Ok(true)
        } else {
            match self.redis_auth {
                Some(redis) => Ok(redis.auth(password).await?),
                None => Ok(false),
            }
        }
    }

    async fn stat(&self, password: String, upload: u64, download: u64) -> Result<()> {
        if self.config_auth.auth(password).await? {
            Ok(self.config_auth.stat(password, upload, download).await?)
        } else {
            match self.redis_auth {
                Some(redis) => {
                    if redis.auth(password).await? {
                        Ok(redis.stat(password, upload, download).await?)
                    } else {
                        Err(anyhow!("User {} not found.", &password))
                    }
                }
                None => Err(anyhow!("User {} not found.", &password)),
            }
        }
    }
}
