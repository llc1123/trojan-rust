use crate::utils::config::Tls;
use anyhow::{anyhow, bail, Context, Result};
use std::{fs::File, io::BufReader, path::Path, sync::Arc};
use tokio_rustls::{
    rustls::{
        internal::pemfile::{certs, pkcs8_private_keys, rsa_private_keys},
        Certificate, KeyLogFile, NoClientAuth, PrivateKey, ServerConfig,
    },
    TlsAcceptor,
};

fn load_certs(path: &Path) -> Result<Vec<Certificate>> {
    let ct =
        certs(&mut BufReader::new(File::open(path)?)).map_err(|_| anyhow!("Invalid cert file"))?;
    if ct.len() == 0 {
        bail!("No valid certs found in file {}", path.display());
    }
    Ok(ct)
}

fn load_keys(path: &Path) -> Result<Vec<PrivateKey>> {
    let pkcs8 = pkcs8_private_keys(&mut BufReader::new(File::open(path)?))
        .map_err(|_| anyhow!("Invalid key file"))?;
    if pkcs8.len() != 0 {
        return Ok(pkcs8);
    }
    let rsa = rsa_private_keys(&mut BufReader::new(File::open(path)?))
        .map_err(|_| anyhow!("Invalid key file"))?;
    if rsa.len() != 0 {
        return Ok(rsa);
    }
    bail!("No valid key found in file {}", path.display())
}

pub fn from(config: &Tls) -> Result<TlsAcceptor> {
    let certs = load_certs(Path::new(&config.cert))
        .context(format!("Failed to load certs from {}", &config.cert))?;
    let mut keys = load_keys(Path::new(&config.key))
        .context(format!("Failed to load privkey from {}", &config.key))?;
    let mut server_config = ServerConfig::new(NoClientAuth::new());
    server_config
        .set_single_cert(certs, keys.remove(0))
        .context("Invalid server config.")?;
    server_config.key_log = Arc::new(KeyLogFile::new());
    let acceptor = TlsAcceptor::from(Arc::new(server_config));

    Ok(acceptor)
}
