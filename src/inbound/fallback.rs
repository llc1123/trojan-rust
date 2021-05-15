use anyhow::{Context, Result};
use http::{Method, Request, Response, StatusCode};
use hyper::{server::conn::Http, service::service_fn, Body};
use log::info;
use tokio::{
    io::{copy_bidirectional, AsyncRead, AsyncWrite},
    net::TcpStream,
};

pub struct FallbackAcceptor {
    // to a http server, if it's empty, fallback will use builtin http server.
    target: String,
}

impl FallbackAcceptor {
    pub async fn new(target: String) -> Result<FallbackAcceptor> {
        if target.len() != 0 {
            TcpStream::connect(&target)
                .await
                .context(format!("Fallback server {} is unavailable.", &target))?;
            info!("Using fallback {}.", &target);
        } else {
            info!("Using built-in fallback server.")
        }
        Ok(FallbackAcceptor { target })
    }
    pub async fn accept<IO>(&self, stream: IO) -> Result<()>
    where
        IO: AsyncRead + AsyncWrite + Unpin + 'static,
    {
        if self.target.len() == 0 {
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
            .await
            .context("Built-in fallback error.")?;

        Ok(())
    }
    async fn handle_forward<IO>(&self, mut stream: IO) -> Result<()>
    where
        IO: AsyncRead + AsyncWrite + Unpin,
    {
        let mut outbound = TcpStream::connect(&self.target)
            .await
            .context("Cannot connect to fallback.")?;

        copy_bidirectional(&mut outbound, &mut stream)
            .await
            .context("Cannot redirect stream to fallback.")?;

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
