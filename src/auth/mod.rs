pub mod from_config;
pub mod redis;

use self::{from_config::ConfigAuthenticator, redis::RedisAuthenticator};
use crate::utils::config::ServerConfig;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;

#[async_trait]
pub trait Auth: Send + Sync + Unpin {
    async fn auth(&self, password: &str) -> Result<bool>;
    async fn stat(&self, password: &str, upload: u64, download: u64) -> Result<()>;
}

pub struct AuthHub {
    config_auth: ConfigAuthenticator,
    redis_auth: Option<RedisAuthenticator>,
}

impl AuthHub {
    pub async fn new(config: &ServerConfig) -> Result<AuthHub> {
        let config_auth = ConfigAuthenticator::new(config.trojan.password.clone())?;
        let mut redis_auth: Option<RedisAuthenticator> = None;
        if let Some(redis) = &config.redis {
            redis_auth = Some(
                RedisAuthenticator::new(&redis.server)
                    .await
                    .context(format!("Unable to connect redis server {}", &redis.server))?,
            );
        }
        Ok(AuthHub {
            config_auth,
            redis_auth,
        })
    }
}

#[async_trait]
impl Auth for AuthHub {
    async fn auth(&self, password: &str) -> Result<bool> {
        if self.config_auth.auth(&password).await? {
            return Ok(true);
        }
        if let Some(redis) = &self.redis_auth {
            return Ok(redis.auth(password).await?);
        }
        Ok(false)
    }

    async fn stat(&self, password: &str, upload: u64, download: u64) -> Result<()> {
        if self.config_auth.auth(password).await? {
            return Ok(self.config_auth.stat(password, upload, download).await?);
        }
        if let Some(redis) = &self.redis_auth {
            if redis.auth(password).await? {
                return Ok(redis.stat(password, upload, download).await?);
            }
        }
        Err(anyhow!("User {} not found.", &password))
    }
}
