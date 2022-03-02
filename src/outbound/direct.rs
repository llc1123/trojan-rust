use std::{
    io,
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};

use crate::common::{Address, AddressDomain, AsyncUdp};

use super::Outbound;
use async_trait::async_trait;
use futures::{ready, Future, FutureExt, TryFutureExt};
use tokio::{
    io::ReadBuf,
    net::{lookup_host, TcpStream, UdpSocket},
    sync::Mutex,
};

pub struct DirectOutbound {}

impl DirectOutbound {
    pub fn new() -> Self {
        DirectOutbound {}
    }
}

#[async_trait]
impl Outbound for DirectOutbound {
    type TcpStream = TcpStream;
    type UdpSocket = Udp;

    async fn tcp_connect(&self, addr: &Address) -> io::Result<Self::TcpStream> {
        match addr {
            Address::SocketAddr(addr) => TcpStream::connect(addr).await,
            Address::Domain(AddressDomain(domain, port)) => {
                TcpStream::connect((domain.as_ref(), *port)).await
            }
        }
    }

    async fn udp_bind(&self, addr: &Address) -> io::Result<Self::UdpSocket> {
        let udp = match addr {
            Address::SocketAddr(addr) => UdpSocket::bind(addr).await,
            Address::Domain(AddressDomain(domain, port)) => {
                UdpSocket::bind((domain.as_ref(), *port)).await
            }
        }?;
        Ok(Udp {
            inner: udp,
            state: UdpState::Idle,
        })
    }
}

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;
enum UdpState {
    Idle,
    LookupHost(Mutex<BoxFuture<io::Result<Option<SocketAddr>>>>),
    Sending(SocketAddr),
}

struct Udp {
    inner: UdpSocket,
    state: UdpState,
}

impl AsyncUdp for Udp {
    fn poll_recv_from(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<SocketAddr>> {
        self.inner.poll_recv_from(cx, buf)
    }

    fn poll_send_to(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        addr: &Address,
    ) -> Poll<io::Result<usize>> {
        let Udp { inner, state, .. } = self;

        loop {
            match state {
                UdpState::Idle => match addr {
                    Address::SocketAddr(s) => {
                        self.state = UdpState::Sending(*s);
                    }
                    Address::Domain(AddressDomain(domain, port)) => {
                        let fut = Mutex::new(
                            lookup_host((domain.clone(), *port))
                                .map_ok(|i| i.next())
                                .boxed(),
                        );
                        self.state = UdpState::LookupHost(fut);
                    }
                },
                UdpState::LookupHost(fut) => {
                    let addr = ready!(fut.get_mut().poll_unpin(cx))?
                        .ok_or_else(|| io::Error::from(io::ErrorKind::AddrNotAvailable))?;
                    *state = UdpState::Sending(addr)
                }
                UdpState::Sending(addr) => {
                    ready!(inner.poll_send_to(cx, buf, *addr)?);
                    *state = UdpState::Idle;
                    return Poll::Ready(Ok(buf.len()));
                }
            }
        }
    }
}
