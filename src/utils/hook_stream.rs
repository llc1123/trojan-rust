use futures::ready;
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub struct HookStream<S> {
    inner: S,
    on_read_done: Box<dyn FnMut(&[u8]) + Send + Sync>,
    on_write_done: Box<dyn FnMut(&[u8]) + Send + Sync>,
}

impl<S> AsyncRead for HookStream<S>
where
    S: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        let before = buf.filled().len();

        let result = ready!(Pin::new(&mut self.inner).poll_read(cx, buf))?;

        (self.on_read_done)(&buf.filled()[before..]);

        Poll::Ready(Ok(result))
    }
}

impl<S> AsyncWrite for HookStream<S>
where
    S: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let result = ready!(Pin::new(&mut self.inner).poll_write(cx, buf))?;

        (self.on_write_done)(&buf[..result]);

        Poll::Ready(Ok(result))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl<S> HookStream<S> {
    pub fn new(
        inner: S,
        on_read_done: impl FnMut(&[u8]) + Send + Sync + 'static,
        on_write_done: impl FnMut(&[u8]) + Send + Sync + 'static,
    ) -> Self {
        HookStream {
            inner,
            on_read_done: Box::new(on_read_done),
            on_write_done: Box::new(on_write_done),
        }
    }
}
