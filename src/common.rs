use futures::future::poll_fn;
use std::{
    fmt, io,
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub trait AsyncTcp {
    fn poll_read(&mut self, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>>;
    fn poll_write(&mut self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>>;
    fn poll_flush(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>>;
    fn poll_shutdown(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>>;
}

pub struct TcpStream<T>(pub T);

impl<T> AsyncRead for TcpStream<T>
where
    T: AsyncTcp,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.0.poll_read(cx, buf)
    }
}

impl<T> AsyncWrite for TcpStream<T>
where
    T: AsyncTcp,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.0.poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.0.poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.0.poll_shutdown(cx)
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> AsyncTcp for T {
    fn poll_read(&mut self, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        AsyncRead::poll_read(Pin::new(&mut self), cx, buf)
    }

    fn poll_write(&mut self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        AsyncWrite::poll_write(Pin::new(&mut self), cx, buf)
    }

    fn poll_flush(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        AsyncWrite::poll_flush(Pin::new(&mut self), cx)
    }

    fn poll_shutdown(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        AsyncWrite::poll_shutdown(Pin::new(&mut self), cx)
    }
}

#[derive(Debug, Clone)]
pub struct AddressDomain(pub String, pub u16);

#[derive(Debug, Clone)]
pub enum Address {
    Domain(AddressDomain),
    SocketAddr(SocketAddr),
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Address::Domain(AddressDomain(domain, port)) => write!(f, "{}:{}", domain, port),
            Address::SocketAddr(s) => write!(f, "{}", s),
        }
    }
}

impl From<socks5_protocol::Address> for Address {
    fn from(addr: socks5_protocol::Address) -> Self {
        match addr {
            socks5_protocol::Address::SocketAddr(s) => Address::SocketAddr(s),
            socks5_protocol::Address::Domain(d, p) => Address::Domain(AddressDomain(d, p)),
        }
    }
}

impl From<Address> for socks5_protocol::Address {
    fn from(addr: Address) -> Self {
        match addr {
            Address::SocketAddr(s) => socks5_protocol::Address::SocketAddr(s),
            Address::Domain(AddressDomain(d, p)) => socks5_protocol::Address::Domain(d, p),
        }
    }
}

impl From<SocketAddr> for Address {
    fn from(addr: SocketAddr) -> Self {
        Address::SocketAddr(addr)
    }
}

impl Address {
    pub fn to_socks5_addr(&self) -> socks5_protocol::Address {
        match self {
            Address::Domain(AddressDomain(domain, port)) => {
                socks5_protocol::Address::Domain(domain.clone(), *port)
            }
            Address::SocketAddr(s) => socks5_protocol::Address::SocketAddr(*s),
        }
    }
}

pub trait AsyncUdp {
    fn poll_recv_from(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<SocketAddr>>;
    fn poll_send_to(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        addr: &Address,
    ) -> Poll<io::Result<usize>>;
}

pub struct UdpStream(Box<dyn AsyncUdp + Send + Sync + 'static>);

impl UdpStream {
    pub fn new(udp: impl AsyncUdp + Send + Sync + 'static) -> Self {
        UdpStream(Box::new(udp))
    }
    pub async fn recv_from(&mut self, buf: &mut ReadBuf<'_>) -> io::Result<SocketAddr> {
        poll_fn(|cx| self.0.poll_recv_from(cx, buf)).await
    }
    pub async fn send_to(&mut self, buf: &[u8], addr: &Address) -> io::Result<usize> {
        poll_fn(|cx| self.0.poll_send_to(cx, buf, addr)).await
    }
    pub fn as_ref(&self) -> &dyn AsyncUdp {
        &*self.0
    }
    pub fn as_mut(&mut self) -> &mut dyn AsyncUdp {
        &mut *self.0
    }
}

impl<T> TcpStream<T> {
    pub fn as_ref(&self) -> &T {
        &self.0
    }
    pub fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

pub type BoxTcpStream = TcpStream<Box<dyn AsyncTcp + Send + Sync + Unpin + 'static>>;

impl BoxTcpStream {
    pub fn boxed(tcp: impl AsyncTcp + Send + Sync + Unpin + 'static) -> Self {
        TcpStream(Box::new(tcp))
    }
}

impl AsyncTcp for BoxTcpStream {
    fn poll_read(&mut self, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        self.0.poll_read(cx, buf)
    }

    fn poll_write(&mut self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.0.poll_write(cx, buf)
    }

    fn poll_flush(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.0.poll_flush(cx)
    }

    fn poll_shutdown(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.0.poll_shutdown(cx)
    }
}
