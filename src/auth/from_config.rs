use super::Auth;
use anyhow::Result;
use async_trait::async_trait;
use sha2::{Digest, Sha224};
use std::collections::HashSet;

pub struct ConfigAuthenticator {
    store: HashSet<String>,
}

impl ConfigAuthenticator {
    pub fn new(passwords: Vec<String>) -> Result<ConfigAuthenticator> {
        let mut s: HashSet<String> = HashSet::new();
        for p in passwords {
            let mut hasher = Sha224::new();
            hasher.update(p.into_bytes());
            let result = hasher.finalize();
            s.insert(hex::encode(result));
        }
        Ok(ConfigAuthenticator { store: s })
    }
}

#[async_trait]
impl Auth for ConfigAuthenticator {
    async fn auth(&self, password: &str) -> Result<bool> {
        Ok(self.store.contains(password))
    }

    async fn stat(&self, password: &str, _: u64, _: u64) -> Result<()> {
        self.auth(password).await?;
        Ok(())
    }
}
