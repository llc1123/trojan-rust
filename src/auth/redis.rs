use super::Auth;
use anyhow::{Context, Result};
use async_trait::async_trait;
use redis::AsyncCommands;
use std::sync::Arc;

#[derive(Clone)]
pub struct RedisAuthenticator {
    client: Arc<redis::Client>,
}

impl RedisAuthenticator {
    pub fn new(server: String) -> Result<RedisAuthenticator> {
        let client = redis::Client::open(format!("redis://{}/", &server))
            .context(format!("Redis server {} unavailable.", &server))?;
        Ok(RedisAuthenticator {
            client: Arc::new(client),
        })
    }
}

#[async_trait]
impl Auth for RedisAuthenticator {
    async fn auth(&self, password: &str) -> Result<bool> {
        let mut con = self
            .client
            .get_async_connection()
            .await
            .context("Cannot create connection to redis server.")?;
        Ok(con
            .exists::<_, bool>(password)
            .await
            .context("Executing command EXISTS failed.")?)
    }

    async fn stat(&self, password: &str, upload: u64, download: u64) -> Result<()> {
        let mut con = self
            .client
            .get_async_connection()
            .await
            .context("Cannot create connection to redis server.")?;
        Ok(redis::pipe()
            .atomic()
            .hincr(password, "upload", upload)
            .hincr(password, "download", download)
            .query_async::<_, ()>(&mut con)
            .await
            .context("Executing command MULTI failed.")?)
    }
}
