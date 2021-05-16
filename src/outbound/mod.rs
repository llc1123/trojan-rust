pub mod direct;

use futures::{Sink, Stream};
use std::{io, net::SocketAddr};
use tokio::net::TcpStream;

pub type UdpPacket = (Vec<u8>, SocketAddr);
pub trait UdpStream: Stream<Item = UdpPacket> + Sink<UdpPacket, Error = io::Error> + Unpin {}
pub type BoxedUdpStream = Box<dyn UdpStream>;

pub enum OutboundStream {
    Tcp(TcpStream, String),
    Udp(BoxedUdpStream),
}
