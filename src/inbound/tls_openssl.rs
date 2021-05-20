use crate::utils::config::Tls;
use anyhow::Result;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod, SslRef, SslVerifyMode};

fn keylog_callback(ssl: &SslRef, s: &str) {}

pub fn new(config: &Tls) -> Result<SslAcceptor> {
    let mut acceptor = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls_server())?;
    acceptor.set_verify(SslVerifyMode::NONE);
    acceptor.set_certificate_chain_file(&config.cert)?;
    acceptor.set_private_key_file(&config.key, SslFiletype::PEM)?;
    acceptor.check_private_key()?;
    acceptor.set_keylog_callback(keylog_callback);

    Ok(acceptor.build())
}
