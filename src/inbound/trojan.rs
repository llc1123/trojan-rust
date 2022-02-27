use super::{tls::TlsContext, Inbound, InboundRequest};
use crate::common::UdpStream;
use crate::{
    auth::Auth,
    common::{AsyncTcp, AsyncUdp, BoxTcpStream, TcpStream},
    config::server::Trojan,
    inbound::tls::TlsAccept,
    outbound::Outbound,
    trojan::TrojanUdp,
    utils::peekable_stream::PeekableStream,
};
use anyhow::{bail, Context as ErrContext, Result};
use async_trait::async_trait;
use bytes::Buf;
use fallback::FallbackAcceptor;
use futures::{ready, SinkExt, StreamExt};
use log::{info, warn};
use socks5_protocol::{sync::FromIO, Address};
use std::{
    io::{self, Cursor},
    net::SocketAddr,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpListener,
    sync::mpsc::Sender,
};
use tokio_util::codec::Framed;

mod fallback;

const CMD: usize = 58;
const ATYP: usize = 59;
const DOMAIN_LEN: usize = 60;

type Request<IO> = InboundRequest<BoxTcpStream, TrojanUdp<PeekableStream<IO>>>;

pub enum Cmd {
    Connect(Address),
    UdpAssociate(Address),
}

pub struct TrojanInbound<T> {
    outbound: T,
    auth: Arc<dyn Auth>,
    bind: String,
    tls_context: TlsContext,
    fallback_acceptor: FallbackAcceptor,
}

fn map_err(e: anyhow::Error) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e)
}

#[async_trait]
impl<T> Inbound<T> for TrojanInbound<T>
where
    T: Outbound,
{
    type TcpStream = BoxTcpStream;
    type UdpSocket = UdpStream;

    async fn run(
        &mut self,
        sender: Sender<InboundRequest<Self::TcpStream, Self::UdpSocket>>,
    ) -> Result<()> {
        let listener = TcpListener::bind(&self.bind).await?;

        loop {
            let (stream, from_addr) = listener.accept().await?;
        }
    }
}

impl<O> TrojanInbound<O>
where
    O: Outbound,
{
    pub async fn new(
        outbound: O,
        auth: Arc<dyn Auth>,
        tls_context: TlsContext,
        config: Trojan,
    ) -> Result<Self> {
        let fallback_acceptor = FallbackAcceptor::new(config.fallback)
            .await
            .context("Failed to setup fallback server.")?;
        Ok(TrojanInbound {
            outbound,
            bind: config.bind,
            auth,
            tls_context,
            fallback_acceptor,
        })
    }

    async fn accept<T>(&self, stream: T) -> Result<BoxTcpStream>
    where
        T: AsyncTcp + Send + Sync + Unpin,
    {
        let TlsAccept {
            stream,
            sni_matched,
        } = self.tls_context.accept(stream).await?;

        if sni_matched {
            match self.accept2(stream).await {
                Ok(out) => return Ok(Some(out)),
                Err(stream) => {
                    self.fallback_acceptor
                        .accept(stream)
                        .await
                        .map_err(map_err)?;
                    Ok(None)
                }
            }
        } else {
            warn!("Redirect to fallback: SNI mismatch.");
            self.fallback_acceptor
                .accept(stream)
                .await
                .map_err(map_err)?;
            Ok(None)
        }
    }

    pub async fn accept2<IO>(&self, stream: IO) -> Result<Request<IO>, PeekableStream<IO>>
    where
        IO: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    {
        let mut stream = PeekableStream::new(stream);
        match self.accept_trojan(&mut stream).await {
            Ok((Cmd::Connect(addr), pw)) => Ok(InboundRequest::TcpConnect {
                addr: addr.into(),
                stream: TcpStream::boxed(stream),
            }),
            Ok((Cmd::UdpAssociate(addr), pw)) => Ok(InboundRequest::UdpBind {
                addr: addr.into(),
                stream: TrojanUdp::new(stream, None),
            }),
            Err(e) => {
                warn!("Redirect to fallback: {:?}", e);
                return Err(stream);
            }
        }
    }

    async fn accept_trojan<IO>(&self, stream: &mut PeekableStream<IO>) -> Result<(Cmd, String)>
    where
        IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let mut buf = vec![0u8; 56 + 2 + 2 + 1];
        stream.peek_exact(&mut buf).await?;

        let password = String::from_utf8_lossy(&buf[0..56]);
        if let Err(_) = hex::decode(password.as_ref()) {
            bail!("Not trojan request.")
        }
        if !self.auth.auth(&password).await? {
            bail!("{}", &password)
        }
        let password = password.to_string();

        info!("Trojan request accepted: {}", &password);

        buf.resize(Self::calc_length(&buf)?, 0);
        stream.peek_exact(&mut buf).await?;

        let cmd = buf[CMD];
        let mut reader = Cursor::new(buf);

        // read address
        reader.advance(ATYP);
        let address = Address::read_from(&mut reader)?;
        let end = reader.position() + 2;
        stream.drain(end as usize).await?;

        Ok(match cmd {
            1 => (Cmd::Connect(address), password),
            3 => (Cmd::UdpAssociate(address), password),
            _ => bail!("Unknown command."),
        })
    }

    // length of head must be 61
    fn calc_length(head: &[u8]) -> Result<usize> {
        let len =
            60 + match head[ATYP] {
                // ipv4
                1 => 8,
                // domain
                3 => 1 + head[DOMAIN_LEN] + 2,
                // ipv6
                4 => 18,
                _ => bail!("Unsupported atyp"),
            } + 2;
        Ok(len as usize)
    }
}
