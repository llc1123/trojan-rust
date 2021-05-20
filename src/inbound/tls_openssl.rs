use crate::utils::config::Tls;
use anyhow::Result;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod, SslRef, SslVerifyMode};

fn keylog_callback(ssl: &SslRef, s: &str) {}

pub fn new(config: &Tls) -> Result<SslAcceptor> {
    let mut builder = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls_server())?;
    builder.set_verify(SslVerifyMode::NONE);
    builder.set_certificate_chain_file(&config.cert)?;
    builder.set_private_key_file(&config.key, SslFiletype::PEM)?;
    builder.set_keylog_callback(keylog_callback);
    let acceptor = builder.build();

    Ok(acceptor)
}
