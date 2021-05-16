pub mod direct;

use futures::{Sink, Stream};
use std::{io, net::SocketAddr, pin::Pin};
use tokio::io::{AsyncRead, AsyncWrite};

pub type UdpPacket = (Vec<u8>, SocketAddr);
pub trait UdpStream:
    Stream<Item = io::Result<UdpPacket>> + Sink<UdpPacket, Error = io::Error> + Send
{
}
impl<T> UdpStream for T where
    T: Stream<Item = io::Result<UdpPacket>> + Sink<UdpPacket, Error = io::Error> + Send
{
}
pub type BoxedUdpStream = Pin<Box<dyn UdpStream + 'static>>;

pub trait AsyncStream: AsyncRead + AsyncWrite + Send {}
impl<T: AsyncRead + AsyncWrite + Send> AsyncStream for T {}
pub type BoxedStream = Pin<Box<dyn AsyncStream + 'static>>;

pub enum OutboundStream {
    Tcp(BoxedStream, String),
    Udp(BoxedUdpStream),
}
