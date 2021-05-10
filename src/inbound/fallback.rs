use anyhow::Result;
use http::{Method, Request, Response, StatusCode};
use hyper::{server::conn::Http, service::service_fn, Body};
use std::sync::Arc;
use tokio::{
    io::{copy_bidirectional, AsyncRead, AsyncWrite},
    net::TcpStream,
};

pub struct Config {
    // to a http server, if it's empty, fallback will use builtin http server.
    pub target: String,
}

#[derive(Clone)]
pub struct FallbackAcceptor {
    inner: Arc<Config>,
}

impl FallbackAcceptor {
    pub fn new(config: Config) -> Result<FallbackAcceptor> {
        Ok(FallbackAcceptor {
            inner: Arc::new(config),
        })
    }
    pub async fn accept<IO>(&self, stream: IO) -> Result<()>
    where
        IO: AsyncRead + AsyncWrite + Unpin + 'static,
    {
        if self.inner.target.len() == 0 {
            self.handle_builtin(stream).await
        } else {
            self.handle_forward(stream).await
        }
    }
    async fn handle_builtin<IO>(&self, stream: IO) -> Result<()>
    where
        IO: AsyncRead + AsyncWrite + Unpin + 'static,
    {
        Http::new()
            .http1_only(true)
            .http1_keep_alive(true)
            .serve_connection(stream, service_fn(hello))
            .await?;

        Ok(())
    }
    async fn handle_forward<IO>(&self, mut stream: IO) -> Result<()>
    where
        IO: AsyncRead + AsyncWrite + Unpin,
    {
        let mut outbound = TcpStream::connect(&self.inner.target).await?;

        copy_bidirectional(&mut outbound, &mut stream).await?;

        Ok(())
    }
}

async fn hello(req: Request<Body>) -> Result<Response<Body>, http::Error> {
    if req.method() == Method::GET {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
    } else {
        Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(Body::empty())
    }
}
