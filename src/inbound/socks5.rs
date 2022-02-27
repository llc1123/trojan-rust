use std::{
    io::{self, Read},
    net::SocketAddr,
    task::{Context, Poll},
};

use anyhow::Result;
use async_trait::async_trait;
use futures::ready;
use socks5_protocol::{
    sync::FromIO, Address, AuthMethod, AuthRequest, AuthResponse, Command, CommandRequest,
    CommandResponse, Error, Version,
};
use tokio::{
    io::ReadBuf,
    net::{TcpListener, TcpStream},
    sync::mpsc::Sender,
};

use crate::{common::AsyncUdp, config, outbound::Outbound};

use super::{Inbound, InboundRequest};

pub struct Socks5Inbound<T> {
    bind: String,
    outbound: T,
}

impl<T> Socks5Inbound<T> {
    pub fn new(outbound: T, config: config::Socks5Inbound) -> Self {
        Socks5Inbound {
            bind: config.bind,
            outbound,
        }
    }
}

#[async_trait]
impl<T> Inbound<T> for Socks5Inbound<T>
where
    T: Outbound,
{
    type TcpStream = T::TcpStream;
    type UdpSocket = Socks5UdpSocket<T::UdpSocket>;

    async fn run(
        &mut self,
        sender: Sender<InboundRequest<Self::TcpStream, Self::UdpSocket>>,
    ) -> Result<()> {
        let listener = TcpListener::bind(self.bind).await?;
        loop {
            listener.accept().await?;
        }
    }
}

pub fn parse_udp(buf: &[u8]) -> io::Result<(Address, &[u8])> {
    let mut cursor = std::io::Cursor::new(buf);
    let mut header = [0u8; 3];
    cursor.read_exact(&mut header)?;
    let addr = match header[0..3] {
        // TODO: support fragment sequence or at least give another error
        [0x00, 0x00, 0x00] => Address::read_from(&mut cursor).map_err(|e| e.to_io_err())?,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "server response wrong RSV {} RSV {} FRAG {}",
                    header[0], header[1], header[2]
                ),
            ))
        }
    };

    let pos = cursor.position() as usize;

    Ok((addr, &cursor.into_inner()[pos..]))
}

pub fn pack_udp(addr: Address, buf: &[u8]) -> io::Result<Vec<u8>> {
    use std::io::Write;

    let mut cursor = std::io::Cursor::new(Vec::new());
    Write::write_all(&mut cursor, &[0x00, 0x00, 0x00])?;
    addr.write_to(&mut cursor).map_err(|e| e.to_io_err())?;
    Write::write_all(&mut cursor, buf)?;

    Ok(cursor.into_inner())
}

pub struct Socks5UdpSocket<U> {
    udp: U,
    _tcp: TcpStream,
    endpoint: Option<SocketAddr>,
    buf: Vec<u8>,
    send_buf: Vec<u8>,
}

impl<U> AsyncUdp for Socks5UdpSocket<U>
where
    U: AsyncUdp,
{
    fn poll_recv_from(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<SocketAddr>> {
        let Socks5UdpSocket {
            udp, endpoint, buf, ..
        } = &mut *self;
        let mut buf = ReadBuf::new(&mut buf[..]);
        let from_addr = ready!(udp.poll_recv_from(cx, &mut buf))?;
        if endpoint.is_none() {
            *endpoint = Some(from_addr);
        };

        let (addr, payload) = parse_udp(&buf.filled())?;

        let to_copy = buf.remaining().min(payload.len());
        buf.initialize_unfilled_to(to_copy)
            .copy_from_slice(&payload[..to_copy]);
        buf.advance(to_copy);

        let addr = match addr {
            Address::SocketAddr(s) => s,
            _ => {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unsupported address type",
                )))
            }
        };
        Poll::Ready(Ok(addr))
    }

    fn poll_send_to(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        addr: SocketAddr,
    ) -> Poll<io::Result<usize>> {
        let Socks5UdpSocket {
            udp,
            endpoint,
            send_buf,
            ..
        } = &mut *self;

        if send_buf.is_empty() {
            let saddr = Address::from(addr);

            let bytes = pack_udp(saddr, buf)?;
            *send_buf = bytes;
        }

        match endpoint {
            Some(endpoint) => {
                ready!(udp.poll_send_to(cx, &send_buf, *endpoint))?;
                send_buf.clear();
            }
            None => {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "udp endpoint not set",
                )))
            }
        }

        Poll::Ready(Ok(buf.len()))
    }
}
