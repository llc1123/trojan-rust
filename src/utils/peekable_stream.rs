#![allow(dead_code)]

use std::{
    collections::VecDeque,
    io,
    pin::Pin,
    task::{Context, Poll},
};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, ReadBuf};

#[derive(Debug)]
pub struct PeekableStream<S> {
    inner: S,
    buf: VecDeque<u8>,
}

impl<S> AsyncRead for PeekableStream<S>
where
    S: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        let (first, ..) = &self.buf.as_slices();
        if first.len() > 0 {
            let read = first.len().min(buf.remaining());
            let unfilled = buf.initialize_unfilled_to(read);
            unfilled[0..read].copy_from_slice(&first[0..read]);
            buf.advance(read);

            // remove 0..read
            self.buf.drain(0..read);

            Poll::Ready(Ok(()))
        } else {
            Pin::new(&mut self.inner).poll_read(cx, buf)
        }
    }
}

impl<S> AsyncWrite for PeekableStream<S>
where
    S: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl<S> PeekableStream<S> {
    pub fn new(inner: S) -> Self {
        PeekableStream {
            inner,
            buf: VecDeque::new(),
        }
    }
    pub fn with_buf(inner: S, buf: VecDeque<u8>) -> Self {
        PeekableStream { inner, buf }
    }
    pub fn into_inner(self) -> (S, VecDeque<u8>) {
        (self.inner, self.buf)
    }
}

impl<S> PeekableStream<S>
where
    S: AsyncRead + Unpin,
{
    // Fill self.buf to size using self.tcp.read_exact
    async fn fill_buf(&mut self, size: usize) -> io::Result<()> {
        if size > self.buf.len() {
            let to_read = size - self.buf.len();
            let mut buf = vec![0u8; to_read];
            self.inner.read_exact(&mut buf).await?;
            self.buf.append(&mut buf.into());
        }
        Ok(())
    }
    pub async fn peek_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.fill_buf(buf.len()).await?;
        let self_buf = self.buf.make_contiguous();
        buf.copy_from_slice(&self_buf[0..buf.len()]);

        Ok(())
    }
    pub async fn drain(&mut self, size: usize) -> io::Result<()> {
        self.fill_buf(size).await?;
        self.buf.drain(0..size);
        Ok(())
    }
}
