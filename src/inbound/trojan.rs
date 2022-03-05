use super::{tls::TlsContext, Inbound, InboundRequest};
use crate::common::{self, UdpStream};
use crate::{
    auth::Auth,
    common::{AsyncTcp, AsyncUdp, BoxTcpStream},
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
    net::{TcpListener, TcpStream},
    sync::mpsc::Sender,
};
use tokio_openssl::SslStream;
use tokio_util::codec::Framed;

mod fallback;

const CMD: usize = 58;
const ATYP: usize = 59;
const DOMAIN_LEN: usize = 60;

type Request = InboundRequest<PeekableStream<TlsStream>, TrojanUdp<PeekableStream<TlsStream>>>;
type TlsStream = SslStream<common::TcpStream<TcpStream>>;

pub enum Cmd {
    Connect(Address),
    UdpAssociate(Address),
}

struct Inner {
    auth: Box<dyn Auth>,
    tls_context: TlsContext,
    fallback_acceptor: FallbackAcceptor,
}

pub struct TrojanInbound {
    bind: String,
    inner: Arc<Inner>,
}

fn map_err(e: anyhow::Error) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e)
}

#[async_trait]
impl Inbound for TrojanInbound {
    type TcpStream = PeekableStream<TlsStream>;
    type UdpSocket = TrojanUdp<PeekableStream<TlsStream>>;

    async fn run(&mut self, sender: Sender<Request>) -> Result<()> {
        let listener = TcpListener::bind(&self.bind).await?;

        loop {
            let (stream, from_addr) = listener.accept().await?;
            let inner = self.inner.clone();
            let sender = sender.clone();
            tokio::spawn(async move { inner.accept_tls(stream, from_addr, sender).await });
        }
    }
}

impl Inner {
    async fn accept_tls(
        &self,
        stream: TcpStream,
        addr: SocketAddr,
        sender: Sender<Request>,
    ) -> Result<()> {
        let TlsAccept {
            stream,
            sni_matched,
        } = self.tls_context.accept(stream).await?;

        if sni_matched {
            match self.accept_trojan(stream).await {
                Ok(out) => {
                    sender.send(out);
                }
                Err(stream) => {
                    self.fallback_acceptor
                        .accept(stream)
                        .await
                        .map_err(map_err)?;
                }
            }
        } else {
            warn!("Redirect to fallback: SNI mismatch.");
            self.fallback_acceptor
                .accept(stream)
                .await
                .map_err(map_err)?;
        }

        Ok(())
    }

    pub async fn accept_trojan(
        &self,
        stream: TlsStream,
    ) -> Result<Request, PeekableStream<TlsStream>> {
        let mut stream = PeekableStream::new(stream);
        match self.accept_trojan_cmd(&mut stream).await {
            Ok((Cmd::Connect(addr), pw)) => Ok(InboundRequest::TcpConnect {
                addr: addr.into(),
                stream,
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

    async fn accept_trojan_cmd(
        &self,
        stream: &mut PeekableStream<TlsStream>,
    ) -> Result<(Cmd, String)> {
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

        buf.resize(calc_length(&buf)?, 0);
        stream.peek_exact(&mut buf).await?;

        let cmd = buf[CMD];
        let mut reader = Cursor::new(buf);

        // read address
        reader.advance(ATYP);
        let address = Address::read_from(&mut reader)?;
        let end = reader.position() + 2;
        stream.drain(end as usize).await?;

        let cmd = match cmd {
            1 => Cmd::Connect(address),
            3 => Cmd::UdpAssociate(address),
            _ => bail!("Unknown command."),
        };

        Ok((cmd, password))
    }
}

impl TrojanInbound {
    pub async fn new(auth: Box<dyn Auth>, tls_context: TlsContext, config: Trojan) -> Result<Self> {
        let fallback_acceptor = FallbackAcceptor::new(config.fallback)
            .await
            .context("Failed to setup fallback server.")?;
        Ok(TrojanInbound {
            bind: config.bind,
            inner: Arc::new(Inner {
                auth,
                tls_context,
                fallback_acceptor,
            }),
        })
    }
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
