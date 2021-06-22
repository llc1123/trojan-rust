use super::Auth;
use anyhow::{Context, Result};
use async_trait::async_trait;
use log::info;
use redis::aio::ConnectionManager;

pub struct RedisAuthenticator {
    client: ConnectionManager,
}

impl RedisAuthenticator {
    pub async fn new(server: &str) -> Result<RedisAuthenticator> {
        let client = redis::Client::open(format!("redis://{}/", server))?;
        let client = ConnectionManager::new(client)
            .await
            .context("Cannot create connection to redis server.")?;
        info!("Using redis auth: {}", server);
        Ok(RedisAuthenticator { client })
    }
}

#[async_trait]
impl Auth for RedisAuthenticator {
    async fn auth(&self, password: &str) -> Result<bool> {
        let mut client = self.client.clone();
        Ok(redis::Cmd::exists(password)
            .query_async(&mut client)
            .await
            .context("Executing command EXISTS failed.")?)
    }

    async fn stat(&self, password: &str, upload: u64, download: u64) -> Result<()> {
        let mut client = self.client.clone();
        Ok(redis::pipe()
            .atomic()
            .hincr(password, "upload", upload)
            .hincr(password, "download", download)
            .query_async(&mut client)
            .await
            .context("Executing command MULTI failed.")?)
    }
}
