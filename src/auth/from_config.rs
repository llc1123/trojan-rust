use super::Auth;
use anyhow::{Context, Result};
use async_trait::async_trait;
use sha2::{Digest, Sha224};
use std::collections::HashSet;

pub struct ConfigAuthenticator {
    store: HashSet<String>,
}

impl ConfigAuthenticator {
    pub fn new(passwords: Vec<String>) -> Result<ConfigAuthenticator> {
        let mut s: HashSet<String> = HashSet::new();
        let mut hasher = Sha224::new();
        for p in passwords {
            hasher.update(p.into_bytes());
            let result = hasher.finalize();

            s.insert(String::from(
                std::str::from_utf8(&result)
                    .context(format!("Unable to parse password: {}", &p))?,
            ));
        }
        Ok(ConfigAuthenticator { store: s })
    }
}

#[async_trait]
impl Auth for ConfigAuthenticator {
    async fn stat(&self, password: String, _: u64, _: u64) -> Result<()> {
        self.auth(password).await?;
        Ok(())
    }

    async fn auth(&self, password: String) -> Result<bool> {
        Ok(self.store.contains(&password))
    }
}
