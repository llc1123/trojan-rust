use anyhow::{bail, Result};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::utils::peekable_stream::PeekableStream;

#[derive(Clone)]
pub struct TrojanAcceptor {}

impl TrojanAcceptor {
    pub fn new() -> Result<TrojanAcceptor> {
        Ok(TrojanAcceptor {})
    }

    pub async fn accept<IO>(&self, stream: &mut PeekableStream<IO>) -> Result<()>
    where
        IO: AsyncRead + AsyncWrite + Unpin + 'static,
    {
        let mut buf = [0u8; 10];
        stream.peek_exact(&mut buf).await?;
        bail!("todo")
        // Don't forget to stream.drain(10)
    }
}
