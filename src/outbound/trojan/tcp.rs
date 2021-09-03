use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use super::tls::TlsStream;
use futures::ready;
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
};

pub struct TrojanTcp {
    stream: TlsStream<TcpStream>,
    head: Option<Vec<u8>>,
    is_first: bool,
}

impl TrojanTcp {
    pub fn new(stream: TlsStream<TcpStream>, head: Vec<u8>) -> Self {
        Self {
            stream,
            head: Some(head),
            is_first: true,
        }
    }
}

impl AsyncRead for TrojanTcp {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        AsyncRead::poll_read(Pin::new(&mut self.stream), cx, buf)
    }
}

impl AsyncWrite for TrojanTcp {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        loop {
            let Self {
                stream,
                head,
                is_first,
            } = &mut *self;
            let stream = Pin::new(stream);
            let len = match head {
                Some(head) => {
                    if *is_first {
                        head.extend(buf);
                        *is_first = false;
                    }

                    let sent = ready!(stream.poll_write(cx, &head))?;
                    head.drain(..sent);
                    head.len()
                }
                None => break,
            };
            if len == 0 {
                *head = None;
                return Poll::Ready(Ok(buf.len()));
            }
        }

        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}
