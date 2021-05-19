use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use futures::ready;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

enum State {
    Write,
    Flush(usize),
}

pub struct PushingStream<S> {
    inner: S,
    state: State,
}

impl<S> AsyncRead for PushingStream<S>
where
    S: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<S> AsyncWrite for PushingStream<S>
where
    S: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let wrote = loop {
            match self.state {
                State::Write => {
                    let wrote = ready!(Pin::new(&mut self.inner).poll_write(cx, buf))?;
                    self.state = State::Flush(wrote);
                }
                State::Flush(wrote) => {
                    ready!(Pin::new(&mut self.inner).poll_flush(cx))?;
                    self.state = State::Write;
                    break wrote;
                }
            }
        };

        Poll::Ready(Ok(wrote))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl<S> PushingStream<S> {
    pub fn new(inner: S) -> Self {
        PushingStream {
            inner,
            state: State::Write,
        }
    }
}
