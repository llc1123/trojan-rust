pub mod from_config;
pub mod redis;

use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Auth {
    async fn auth(&self, password: String) -> Result<bool>;
    async fn stat(&self, password: String, upload: u64, download: u64) -> Result<()>;
}
