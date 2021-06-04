use futures::{channel::oneshot, ready};
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub type Callback = Box<dyn FnOnce(usize, usize) + Send + Sync>;
pub struct CountStream<S> {
    inner: S,
    read: usize,
    write: usize,
    on_drop: oneshot::Receiver<Callback>,
}

impl<S> Drop for CountStream<S> {
    fn drop(&mut self) {
        if let Ok(Some(f)) = self.on_drop.try_recv() {
            f(self.read, self.write)
        }
    }
}

impl<S> AsyncRead for CountStream<S>
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

        let read = &buf.filled()[before..];
        self.read += read.len();

        Poll::Ready(Ok(()))
    }
}

impl<S> AsyncWrite for CountStream<S>
where
    S: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let result = ready!(Pin::new(&mut self.inner).poll_write(cx, buf))?;

        self.write += result;

        Poll::Ready(Ok(result))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl<S> CountStream<S> {
    pub fn new2(inner: S) -> (Self, oneshot::Sender<Callback>) {
        let (tx, rx) = oneshot::channel();
        (
            CountStream {
                inner,
                read: 0,
                write: 0,
                on_drop: rx,
            },
            tx,
        )
    }
}
