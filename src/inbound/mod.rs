use crate::common::{AsyncStream, UdpStream};
use async_trait::async_trait;
use std::{io, net::SocketAddr};

pub mod socks5;
pub mod tls;
pub mod trojan;

pub enum InboundRequest<T, U> {
    TcpConnect { addr: String, stream: T },
    UdpBind { addr: String, stream: U },
}

pub struct InboundAccept<Metadata, T, U> {
    pub metadata: Metadata,
    pub request: InboundRequest<T, U>,
}

#[async_trait]
pub trait Inbound: Send + Sync {
    type Metadata: Send + 'static;
    type TcpStream: AsyncStream + 'static;
    type UdpSocket: UdpStream + 'static;
    async fn accept<AcceptedStream>(
        &self,
        stream: AcceptedStream,
        addr: SocketAddr,
        local_addr: SocketAddr,
    ) -> io::Result<Option<InboundAccept<Self::Metadata, Self::TcpStream, Self::UdpSocket>>>
    where
        AcceptedStream: AsyncStream + Unpin + 'static;
}
