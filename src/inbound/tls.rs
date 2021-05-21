use crate::utils::config::Tls;
use anyhow::{Context, Result};
use futures::TryFutureExt;
use log::trace;
use openssl::ssl;
use std::{env, ops::Deref, path::Path};
use tokio::io::AsyncWriteExt;
use tokio::{fs::OpenOptions, sync::mpsc};

pub struct SslContext {
    inner: ssl::SslContext,
}

impl SslContext {
    pub fn new(config: &Tls) -> Result<SslContext> {
        let (tx, rx) = mpsc::unbounded_channel::<String>();

        let keylog_callback = move |_: &ssl::SslRef, s: &str| {
            trace!("Keylog: {}", &s);
            if tx.is_closed() {
                return;
            }
            tx.send(String::from(s)).ok();
        };

        tokio::spawn(keylogger(rx).inspect_err(|e| log::error!("keylogger error: {:?}", e)));

        let mut acceptor = ssl::SslAcceptor::mozilla_intermediate_v5(ssl::SslMethod::tls_server())?;
        acceptor.set_verify(ssl::SslVerifyMode::NONE);
        acceptor.set_certificate_chain_file(&config.cert)?;
        acceptor.set_private_key_file(&config.key, ssl::SslFiletype::PEM)?;
        acceptor.check_private_key()?;
        acceptor.set_keylog_callback(keylog_callback);

        Ok(SslContext {
            inner: acceptor.build().into_context(),
        })
    }
}

impl Deref for SslContext {
    type Target = ssl::SslContext;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

async fn keylogger(mut rx: mpsc::UnboundedReceiver<String>) -> Result<()> {
    let path = env::var("SSLKEYLOGFILE").unwrap_or_default();
    if path == "" {
        return Ok(());
    }
    let path = Path::new(&path);
    let mut keylogfile = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .await
        .context("Cannot open keylog file.")?;
    loop {
        if let Some(keylog) = rx.recv().await {
            keylogfile.write_all(keylog.as_bytes()).await?;
            keylogfile.write_all(b"\n").await?;
        } else {
            break;
        }
    }

    Ok(())
}
