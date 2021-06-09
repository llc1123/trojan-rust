use futures::ready;
use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    io::{self, AsyncRead, AsyncWrite},
    time::{sleep, Duration, Instant, Sleep},
};

pin_project! {
    #[derive(Debug)]
    pub struct TimeoutStream<S> {
        #[pin]
        s: S,
        duration: Option<Duration>,
        sleep: Pin<Box<Sleep>>,
    }
}

impl<S> TimeoutStream<S> {
    pub fn new(s: S, duration: Option<Duration>) -> TimeoutStream<S>
    where
        S: AsyncRead + AsyncWrite,
    {
        TimeoutStream {
            s,
            duration,
            sleep: Box::pin(sleep(duration.unwrap_or(Duration::from_secs(1)))),
        }
    }
}

impl<S> AsyncRead for TimeoutStream<S>
where
    S: AsyncRead + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.project();
        match this.duration {
            Some(duration) => match this.s.poll_read(cx, buf) {
                Poll::Ready(r) => {
                    this.sleep.as_mut().reset(Instant::now() + *duration);
                    Poll::Ready(r)
                }
                Poll::Pending => {
                    ready!(this.sleep.as_mut().poll(cx));
                    Poll::Ready(Err(io::ErrorKind::TimedOut.into()))
                }
            },
            None => this.s.poll_read(cx, buf),
        }
    }
}

impl<S> AsyncWrite for TimeoutStream<S>
where
    S: AsyncWrite + Unpin,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let this = self.project();
        match this.duration {
            Some(duration) => match this.s.poll_write(cx, buf) {
                Poll::Ready(r) => {
                    this.sleep.as_mut().reset(Instant::now() + *duration);
                    Poll::Ready(r)
                }
                Poll::Pending => {
                    ready!(this.sleep.as_mut().poll(cx));
                    Poll::Ready(Err(io::ErrorKind::TimedOut.into()))
                }
            },
            None => this.s.poll_write(cx, buf),
        }
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.project().s.poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.project().s.poll_shutdown(cx)
    }
}
