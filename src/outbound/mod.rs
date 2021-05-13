pub mod direct;

use std::{io, net::SocketAddr, pin::Pin};

use futures::{Sink, Stream};
use tokio::net::TcpStream;

pub type UdpPacket = (Vec<u8>, SocketAddr);
pub trait UdpStream: Stream<Item = UdpPacket> + Sink<UdpPacket, Error = io::Error> {}
impl<T> UdpStream for T where T: Stream<Item = UdpPacket> + Sink<UdpPacket, Error = io::Error> {}
pub type BoxedUdpStream = Pin<Box<dyn UdpStream>>;

pub enum OutboundStream {
    Tcp(TcpStream, String),
    Udp(BoxedUdpStream),
}
