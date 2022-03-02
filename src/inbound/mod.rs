use std::{
    io,
    net::SocketAddr,
    task::{Context, Poll},
};

use crate::{
    common::{Address, AsyncTcp},
    outbound::Outbound,
};
use anyhow::Result;
use async_trait::async_trait;
use tokio::{io::ReadBuf, sync::mpsc::Sender};

pub mod socks5;
pub mod tls;
pub mod trojan;

pub trait InboundUdp {
    fn poll_recv_send_to(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<Address>>;
    fn poll_send_recv_from(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        addr: SocketAddr,
    ) -> Poll<io::Result<usize>>;
}

pub enum InboundRequest<T, U> {
    TcpConnect { addr: Address, stream: T },
    UdpBind { addr: Address, stream: U },
}

#[async_trait]
pub trait Inbound<T>: Send + Sync
where
    T: Outbound,
{
    type TcpStream: AsyncTcp + Send + Sync + Unpin + 'static;
    type UdpSocket: InboundUdp + Send + Sync + Unpin + 'static;

    async fn run(
        &mut self,
        sender: Sender<InboundRequest<Self::TcpStream, Self::UdpSocket>>,
    ) -> Result<()>;
}
