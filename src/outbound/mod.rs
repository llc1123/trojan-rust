pub mod direct;

use futures::{Sink, Stream};
use std::{io, net::SocketAddr, pin::Pin};
use tokio::io::{AsyncRead, AsyncWrite};

pub type UdpPacket = (Vec<u8>, SocketAddr);
pub trait UdpStream:
    Stream<Item = io::Result<UdpPacket>> + Sink<UdpPacket, Error = io::Error> + Send + Unpin
{
}
impl<T> UdpStream for T where
    T: Stream<Item = io::Result<UdpPacket>> + Sink<UdpPacket, Error = io::Error> + Send + Unpin
{
}
pub type BoxedUdpStream = Pin<Box<dyn UdpStream + 'static>>;

pub trait AsyncStream: AsyncRead + AsyncWrite + Send + Unpin {}
impl<T: AsyncRead + AsyncWrite + Send + Unpin> AsyncStream for T {}
pub type BoxedStream = Box<dyn AsyncStream + 'static>;

pub enum OutboundStream {
    Tcp(BoxedStream, String),
    Udp(BoxedUdpStream),
}
