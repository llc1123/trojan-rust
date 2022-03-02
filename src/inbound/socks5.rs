use std::{
    io::{self, Read},
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4},
    sync::Arc,
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
    io::{AsyncWriteExt, BufWriter, ReadBuf},
    net::{TcpListener, TcpStream, UdpSocket},
    sync::mpsc::Sender,
};

use crate::{common::AsyncUdp, config, outbound::Outbound};

use super::{Inbound, InboundRequest, InboundUdp};

pub struct Socks5Inbound<O> {
    outbound: Arc<O>,
    bind: String,
}

impl<O> Socks5Inbound<O> {
    pub fn new(outbound: O, config: config::Socks5Inbound) -> Self {
        Socks5Inbound {
            outbound: Arc::new(outbound),
            bind: config.bind,
        }
    }
}

#[async_trait]
impl<O> Inbound<O> for Socks5Inbound<O>
where
    O: Outbound,
{
    type TcpStream = TcpStream;
    type UdpSocket = Socks5UdpSocket;

    async fn run(
        &mut self,
        sender: Sender<InboundRequest<Self::TcpStream, Self::UdpSocket>>,
    ) -> Result<()> {
        let listener = TcpListener::bind(self.bind).await?;
        loop {
            let (stream, addr) = listener.accept().await?;
            tokio::spawn(accept(stream, addr, sender.clone(), self.outbound.clone()));
        }
    }
}

async fn accept<O>(
    stream: TcpStream,
    addr: SocketAddr,
    sender: Sender<InboundRequest<TcpStream, Socks5UdpSocket>>,
    outbound: Arc<O>,
) -> Result<()>
where
    O: Outbound,
{
    let default_addr: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0));
    let mut stream = BufWriter::with_capacity(512, stream);

    let version = Version::read(&mut stream).await.map_err(Error::to_io_err)?;
    let auth_req = AuthRequest::read(&mut stream)
        .await
        .map_err(Error::to_io_err)?;

    let method = auth_req.select_from(&[AuthMethod::Noauth]);
    let auth_resp = AuthResponse::new(method);

    // TODO: do auth here

    version.write(&mut stream).await.map_err(Error::to_io_err)?;
    auth_resp
        .write(&mut stream)
        .await
        .map_err(Error::to_io_err)?;
    stream.flush().await?;

    let cmd_req = CommandRequest::read(&mut stream)
        .await
        .map_err(Error::to_io_err)?;

    let accept = match cmd_req.command {
        Command::Connect => {
            CommandResponse::success(default_addr.into())
                .write(&mut stream)
                .await
                .map_err(Error::to_io_err)?;
            stream.flush().await?;

            InboundRequest::TcpConnect {
                addr: cmd_req.address.to_string(),
                stream,
            }
        }
        Command::UdpAssociate => {
            let bind_addr = match cmd_req.address {
                Address::SocketAddr(SocketAddr::V4(_)) => {
                    SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0)
                }
                Address::SocketAddr(SocketAddr::V6(_)) => {
                    SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), 0)
                }
                _ => return Ok(None),
            };
            let udp = UdpSocket::bind(bind_addr).await?;
            let mut local_addr = addr;
            local_addr.set_port(udp.local_addr()?.port());

            CommandResponse::success(local_addr.into())
                .write(&mut stream)
                .await
                .map_err(Error::to_io_err)?;
            stream.flush().await?;
            InboundRequest::UdpBind {
                addr: "0.0.0.0:0".to_string(),
                stream: Socks5UdpSocket {
                    udp,
                    _tcp: stream,
                    endpoint: None,
                    buf: vec![0u8; 2048],
                    send_buf: None,
                },
            }
        }
        _ => {
            return Ok(None);
        }
    };
    Ok(Some(accept))
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

pub struct Socks5UdpSocket {
    udp: UdpSocket,
    _tcp: TcpStream,
    endpoint: Option<SocketAddr>,
    buf: Vec<u8>,
    send_buf: Vec<u8>,
}

impl InboundUdp for Socks5UdpSocket {
    fn poll_recv_send_to(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<crate::common::Address>> {
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
        Poll::Ready(Ok(addr.into()))
    }

    fn poll_send_recv_from(
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
