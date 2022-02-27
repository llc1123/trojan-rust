use crate::{
    common::{Address, AsyncTcp, AsyncUdp},
    outbound::Outbound,
};
use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc::Sender;

pub mod socks5;
pub mod tls;
pub mod trojan;

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
    type UdpSocket: AsyncUdp + Send + Sync + Unpin + 'static;

    async fn run(
        &mut self,
        sender: Sender<InboundRequest<Self::TcpStream, Self::UdpSocket>>,
    ) -> Result<()>;
}
