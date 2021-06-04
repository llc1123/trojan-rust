use bytes::Bytes;
use futures::{Sink, Stream};
use std::{io, pin::Pin};
use tokio::io::{AsyncRead, AsyncWrite};

pub trait AsyncStream: AsyncRead + AsyncWrite + Send {}
impl<T: AsyncRead + AsyncWrite + Send> AsyncStream for T {}

pub type UdpPacket = (Bytes, String);

pub trait UdpStream:
    Stream<Item = io::Result<UdpPacket>> + Sink<UdpPacket, Error = io::Error> + Send
{
}
impl<T> UdpStream for T where
    T: Stream<Item = io::Result<UdpPacket>> + Sink<UdpPacket, Error = io::Error> + Send
{
}

pub type BoxedUdpStream = Pin<Box<dyn UdpStream + 'static>>;

pub type BoxedStream = Pin<Box<dyn AsyncStream + 'static>>;
