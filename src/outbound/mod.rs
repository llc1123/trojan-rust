use crate::common::{AsyncStream, BoxedUdpStream, UdpStream};
use async_trait::async_trait;
use std::io;

pub mod direct;
pub mod trojan;

#[async_trait]
pub trait Outbound: Send + Sync {
    type TcpStream: AsyncStream + 'static;
    type UdpSocket: UdpStream + 'static;

    async fn tcp_connect(&self, address: &str) -> io::Result<Self::TcpStream>;
    async fn udp_bind(&self, address: &str) -> io::Result<Self::UdpSocket>;
}
