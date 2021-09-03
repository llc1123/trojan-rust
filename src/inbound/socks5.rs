use std::{
    io::{self, Read},
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4},
    pin::Pin,
    str::FromStr,
    task::{Context, Poll},
};

use async_trait::async_trait;
use bytes::Bytes;
use futures::{ready, Sink, Stream};
use socks5_protocol::{
    sync::FromIO, Address, AuthMethod, AuthRequest, AuthResponse, Command, CommandRequest,
    CommandResponse, Error, Version,
};
use tokio::{
    io::{split, AsyncWriteExt, BufWriter, ReadBuf},
    net::UdpSocket,
};

use crate::common::{AsyncStream, BoxedStream, UdpPacket};

use super::{Inbound, InboundAccept, InboundRequest};

pub struct Socks5Inbound {}

impl Socks5Inbound {
    pub fn new() -> Self {
        Socks5Inbound {}
    }
}

#[async_trait]
impl Inbound for Socks5Inbound {
    type Metadata = ();
    type TcpStream = BoxedStream;
    type UdpSocket = Socks5UdpSocket;

    async fn accept<AcceptedStream>(
        &self,
        stream: AcceptedStream,
        _addr: SocketAddr,
        mut local_addr: SocketAddr,
    ) -> io::Result<Option<InboundAccept<Self::Metadata, Self::TcpStream, Self::UdpSocket>>>
    where
        AcceptedStream: AsyncStream + Unpin + 'static,
    {
        let default_addr: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0));
        let (mut rx, tx) = split(stream);
        let mut tx = BufWriter::with_capacity(512, tx);

        let version = Version::read(&mut rx).await.map_err(Error::to_io_err)?;
        let auth_req = AuthRequest::read(&mut rx).await.map_err(Error::to_io_err)?;

        let method = auth_req.select_from(&[AuthMethod::Noauth]);
        let auth_resp = AuthResponse::new(method);

        // TODO: do auth here

        version.write(&mut tx).await.map_err(Error::to_io_err)?;
        auth_resp.write(&mut tx).await.map_err(Error::to_io_err)?;
        tx.flush().await?;

        let cmd_req = CommandRequest::read(&mut rx)
            .await
            .map_err(Error::to_io_err)?;

        let accept = match cmd_req.command {
            Command::Connect => {
                CommandResponse::success(default_addr.into())
                    .write(&mut tx)
                    .await
                    .map_err(Error::to_io_err)?;
                tx.flush().await?;

                let socket = rx.unsplit(tx.into_inner());
                InboundAccept {
                    metadata: (),
                    request: InboundRequest::TcpConnect {
                        addr: cmd_req.address.to_string(),
                        stream: Box::pin(socket) as BoxedStream,
                    },
                }
            }
            Command::UdpAssociate => {
                let bind_addr = match cmd_req.address {
                    Address::SocketAddr(SocketAddr::V4(_)) => {
                        SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), 0)
                    }
                    Address::SocketAddr(SocketAddr::V6(_)) => {
                        SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), 0)
                    }
                    _ => return Ok(None),
                };
                let udp = UdpSocket::bind(bind_addr).await?;
                local_addr.set_port(udp.local_addr()?.port());

                CommandResponse::success(local_addr.into())
                    .write(&mut tx)
                    .await
                    .map_err(Error::to_io_err)?;
                tx.flush().await?;
                let socket = rx.unsplit(tx.into_inner());
                InboundAccept {
                    metadata: (),
                    request: InboundRequest::UdpBind {
                        addr: "0.0.0.0:0".to_string(),
                        stream: Socks5UdpSocket {
                            udp,
                            _tcp: Box::pin(socket),
                            endpoint: None,
                            buf: vec![0u8; 2048],
                            send_buf: None,
                        },
                    },
                }
            }
            _ => {
                return Ok(None);
            }
        };
        Ok(Some(accept))
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

pub struct Socks5UdpSocket {
    udp: UdpSocket,
    _tcp: BoxedStream,
    endpoint: Option<SocketAddr>,
    buf: Vec<u8>,
    send_buf: Option<Vec<u8>>,
}

impl Stream for Socks5UdpSocket {
    type Item = io::Result<UdpPacket>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let Socks5UdpSocket {
            udp, endpoint, buf, ..
        } = &mut *self;
        let mut buf = ReadBuf::new(&mut buf[..]);
        let from_addr = ready!(udp.poll_recv_from(cx, &mut buf))?;
        if endpoint.is_none() {
            *endpoint = Some(from_addr);
        };

        let (addr, payload) = parse_udp(&buf.filled())?;

        Poll::Ready(Some(Ok((
            Bytes::copy_from_slice(payload),
            addr.to_string(),
        ))))
    }
}

impl Sink<UdpPacket> for Socks5UdpSocket {
    type Error = io::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.poll_flush(cx)
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        (bytes, send_to): UdpPacket,
    ) -> Result<(), Self::Error> {
        let saddr = Address::from_str(&send_to).map_err(|e| e.to_io_err())?;

        let bytes = pack_udp(saddr, &bytes)?;

        self.send_buf = Some(bytes);

        Ok(())
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let Socks5UdpSocket {
            send_buf,
            endpoint,
            udp,
            ..
        } = &mut *self;

        match (&send_buf, endpoint) {
            (Some(buf), Some(endpoint)) => {
                ready!(udp.poll_send_to(cx, &buf, *endpoint))?;
            }
            // drop the packet
            _ => (),
        };

        *send_buf = None;

        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}
