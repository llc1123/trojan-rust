use futures::ready;
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub struct HookStream<S> {
    inner: S,
    on_read_done: Option<Box<dyn FnMut(&[u8]) + Send + Sync>>,
    on_write_done: Option<Box<dyn FnMut(&[u8]) + Send + Sync>>,
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

        ready!(Pin::new(&mut self.inner).poll_read(cx, buf))?;

        if let Some(f) = &mut self.on_read_done {
            f(&buf.filled()[before..]);
        }

        Poll::Ready(Ok(()))
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

        if let Some(f) = &mut self.on_write_done {
            f(&buf[..result]);
        }

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
    pub fn new(inner: S) -> Self {
        HookStream {
            inner,
            on_read_done: None,
            on_write_done: None,
        }
    }
    pub fn set_on_read_done(
        &mut self,
        on_read_done: Option<impl FnMut(&[u8]) + Send + Sync + 'static>,
    ) {
        if let Some(f) = on_read_done {
            self.on_read_done = Some(Box::new(f))
        }
    }
    pub fn set_on_write_done(
        &mut self,
        on_write_done: Option<impl FnMut(&[u8]) + Send + Sync + 'static>,
    ) {
        if let Some(f) = on_write_done {
            self.on_write_done = Some(Box::new(f))
        }
    }
}
