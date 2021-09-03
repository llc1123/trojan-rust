use anyhow::Result;
use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use std::{io, pin::Pin};
use tokio::io::{AsyncRead, AsyncWrite};

pub use tokio_openssl::SslStream as TlsStream;

pub struct TlsConnector {
    connector: SslConnector,
    sni: String,
}

impl TlsConnector {
    pub fn new(sni: String, skip_cert_verify: bool) -> Result<TlsConnector> {
        let mut builder = SslConnector::builder(SslMethod::tls())?;

        if skip_cert_verify {
            builder.set_verify(SslVerifyMode::NONE);
        }

        Ok(TlsConnector {
            connector: builder.build(),
            sni,
        })
    }

    pub async fn connect<IO>(&self, stream: IO) -> io::Result<TlsStream<IO>>
    where
        IO: AsyncRead + AsyncWrite + Unpin,
    {
        let ssl = self.connector.configure()?.into_ssl(&self.sni)?;

        let mut stream = TlsStream::new(ssl, stream)?;

        Pin::new(&mut stream)
            .connect()
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        Ok(stream)
    }
}
