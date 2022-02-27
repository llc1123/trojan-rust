use crate::common::{Address, AsyncTcp, AsyncUdp};
use async_trait::async_trait;
use std::io;

pub mod direct;
pub mod trojan;

#[async_trait]
pub trait Outbound: Send + Sync {
    type TcpStream: AsyncTcp + Send + Sync + Unpin + 'static;
    type UdpSocket: AsyncUdp + Send + Sync + Unpin + 'static;

    async fn tcp_connect(&self, addr: &Address) -> io::Result<Self::TcpStream>;
    async fn udp_bind(&self, addr: &Address) -> io::Result<Self::UdpSocket>;
}
