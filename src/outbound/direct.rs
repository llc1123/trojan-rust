use std::{
    io,
    net::SocketAddr,
    task::{Context, Poll},
};

use crate::common::{Address, AddressDomain, AsyncUdp};

use super::Outbound;
use async_trait::async_trait;
use tokio::{
    io::ReadBuf,
    net::{TcpStream, UdpSocket},
};

pub struct DirectOutbound {}

impl DirectOutbound {
    pub fn new() -> Self {
        DirectOutbound {}
    }
}

#[async_trait]
impl Outbound for DirectOutbound {
    type TcpStream = TcpStream;
    type UdpSocket = Udp;

    async fn tcp_connect(&self, addr: &Address) -> io::Result<Self::TcpStream> {
        match addr {
            Address::SocketAddr(addr) => TcpStream::connect(addr).await,
            Address::Domain(AddressDomain(domain, port)) => {
                TcpStream::connect((domain.as_ref(), *port)).await
            }
        }
    }

    async fn udp_bind(&self, addr: &Address) -> io::Result<Self::UdpSocket> {
        let udp = match addr {
            Address::SocketAddr(addr) => UdpSocket::bind(addr).await,
            Address::Domain(AddressDomain(domain, port)) => {
                UdpSocket::bind((domain.as_ref(), *port)).await
            }
        }?;
        Ok(Udp(udp))
    }
}

struct Udp(UdpSocket);

impl AsyncUdp for Udp {
    fn poll_recv_from(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<SocketAddr>> {
        self.0.poll_recv_from(cx, buf)
    }

    fn poll_send_to(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        addr: SocketAddr,
    ) -> Poll<io::Result<usize>> {
        self.0.poll_send_to(cx, buf, addr)
    }
}
