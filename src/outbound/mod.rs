pub mod direct;

use std::net::SocketAddr;

use futures::{Sink, Stream};
use tokio::net::TcpStream;

type UdpPacket = (Vec<u8>, SocketAddr);
trait UdpStream: Stream<Item = UdpPacket> + Sink<UdpPacket, Error = std::io::Error> + Sized {}
impl<T> UdpStream for T where
    T: Stream<Item = UdpPacket> + Sink<UdpPacket, Error = std::io::Error> + Sized
{
}

pub enum OutboundStream {
    Tcp(TcpStream, String),
    Udp(Box<dyn UdpStream>),
}
