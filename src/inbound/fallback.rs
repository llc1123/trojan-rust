use anyhow::Result;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub struct Config {}

#[derive(Clone)]
pub struct FallbackAcceptor {
    inner: Arc<Config>,
}

impl From<Arc<Config>> for FallbackAcceptor {
    fn from(inner: Arc<Config>) -> FallbackAcceptor {
        FallbackAcceptor { inner }
    }
}

impl FallbackAcceptor {
    pub async fn accept<IO>(&self, mut stream: IO) -> Result<()>
    where
        IO: AsyncRead + AsyncWrite + Unpin,
    {
        let mut buffer = [0; 1024];
        stream.read(&mut buffer).await.unwrap_or_default();

        let mut headers = [http_bytes::EMPTY_HEADER; 16];

        let response_400 = "HTTP/1.1 400 Bad Request\r\n\r\n";
        let response_404 = "HTTP/1.1 404 Not Found\r\n\r\n";
        let response_405 = "HTTP/1.1 405 Method Not Allowed\r\n\r\n";

        let response = if let Some((req, _)) = http_bytes::parse_request_header(
            &buffer,
            &mut headers[..],
            Some(http_bytes::http::uri::Scheme::HTTP),
        )? {
            if req.method() == http_bytes::http::method::Method::GET {
                response_404
            } else {
                response_405
            }
        } else {
            response_400
        };

        stream.write(response.as_bytes()).await?;
        stream.shutdown().await?;

        Ok(())
    }
}

pub fn from(config: Config) -> Result<FallbackAcceptor> {
    Ok(FallbackAcceptor::from(Arc::new(config)))
}
