use crate::{common::AsyncStream, utils::config::Tls};
use anyhow::{anyhow, Context, Result};
use futures::TryFutureExt;
use log::{debug, info, trace};
use openssl::ssl::{self, NameType, Ssl, SslContext};
use std::{env, ops::Deref, path::Path, pin::Pin};
use tokio::{fs::OpenOptions, io::AsyncWriteExt, sync::mpsc};
use tokio_openssl::SslStream;

pub struct TlsContext {
    inner: ssl::SslContext,
    sni: String,
}

pub struct TlsAccept<S> {
    pub sni_matched: bool,
    pub stream: SslStream<S>,
}

fn get_alt_names_from_ssl_context(context: &SslContext) -> Option<Vec<String>> {
    if let Some(cert) = context.certificate() {
        if let Some(names) = cert.subject_alt_names() {
            return Some(
                names
                    .iter()
                    .map(|x| x.dnsname())
                    .filter(|x| x.is_some())
                    .map(|x| String::from(x.unwrap()))
                    .collect(),
            );
        }
    }
    None
}

impl TlsContext {
    pub fn new(config: &Tls) -> Result<TlsContext> {
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
        let context = acceptor.build().into_context();

        let names = get_alt_names_from_ssl_context(&context)
            .ok_or(anyhow!("Cannot get domain names from cert."))?;
        info!("{:?}", names);

        Ok(TlsContext {
            inner: context,
            sni: config.sni.clone(),
        })
    }

    pub async fn accept<S: AsyncStream + Unpin>(&self, stream: S) -> Result<TlsAccept<S>> {
        let mut stream = SslStream::new(Ssl::new(&self.inner)?, stream)?;
        Pin::new(&mut stream)
            .accept()
            .await
            .context("Invalid TLS connection.")?;
        let servername = stream
            .ssl()
            .servername(NameType::HOST_NAME)
            .unwrap_or_default();
        debug!("SNI: {:?}", &servername);

        Ok(TlsAccept {
            sni_matched: servername == self.sni,
            stream,
        })
    }
}

impl Deref for TlsContext {
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
