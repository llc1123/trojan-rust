use super::{tls::TlsContext, Inbound, InboundAccept, InboundRequest};
use crate::trojan::UdpCodec;
use crate::{
    auth::Auth,
    common::{AsyncStream, BoxedStream, BoxedUdpStream},
    inbound::tls::TlsAccept,
    utils::{config::server::Trojan, peekable_stream::PeekableStream},
};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use bytes::Buf;
use fallback::FallbackAcceptor;
use log::{info, warn};
use socks5_protocol::{sync::FromIO, Address};
use std::{
    io::{self, Cursor},
    net::SocketAddr,
    sync::Arc,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::Framed;

mod fallback;

const CMD: usize = 58;
const ATYP: usize = 59;
const DOMAIN_LEN: usize = 60;

pub enum Cmd {
    Connect(String),
    UdpAssociate,
}

pub struct TrojanInbound {
    auth_hub: Arc<dyn Auth>,
    tls_context: TlsContext,
    fallback_acceptor: FallbackAcceptor,
}

fn map_err(e: anyhow::Error) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e)
}

#[async_trait]
impl Inbound for TrojanInbound {
    type Metadata = String;
    type TcpStream = BoxedStream;
    type UdpSocket = BoxedUdpStream;

    async fn accept<AcceptedStream>(
        &self,
        stream: AcceptedStream,
        _addr: SocketAddr,
        _local_addr: SocketAddr,
    ) -> io::Result<Option<InboundAccept<Self::Metadata, Self::TcpStream, Self::UdpSocket>>>
    where
        AcceptedStream: AsyncStream + Unpin + 'static,
    {
        let TlsAccept {
            stream,
            sni_matched,
        } = self.tls_context.accept(stream).await.map_err(map_err)?;

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
}

impl TrojanInbound {
    pub async fn new(
        auth: Arc<dyn Auth>,
        tls_context: TlsContext,
        config: Trojan,
    ) -> Result<TrojanInbound> {
        let fallback_acceptor = FallbackAcceptor::new(config.fallback)
            .await
            .context("Failed to setup fallback server.")?;
        Ok(TrojanInbound {
            auth_hub: auth,
            tls_context,
            fallback_acceptor,
        })
    }

    pub async fn accept2<IO>(
        &self,
        stream: IO,
    ) -> Result<InboundAccept<String, BoxedStream, BoxedUdpStream>, PeekableStream<IO>>
    where
        IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let mut stream = PeekableStream::new(stream);
        match self.inner_accept(&mut stream).await {
            Ok((Cmd::Connect(addr), pw)) => Ok(InboundAccept {
                metadata: pw,
                request: InboundRequest::TcpConnect {
                    addr,
                    stream: Box::pin(stream),
                },
            }),
            Ok((Cmd::UdpAssociate, pw)) => Ok(InboundAccept {
                metadata: pw,
                request: InboundRequest::UdpBind {
                    addr: "0.0.0.0:0".to_string(),
                    stream: Box::pin(Framed::new(stream, UdpCodec::new(None))),
                },
            }),
            Err(e) => {
                warn!("Redirect to fallback: {:?}", e);
                return Err(stream);
            }
        }
    }

    async fn inner_accept<IO>(&self, stream: &mut PeekableStream<IO>) -> Result<(Cmd, String)>
    where
        IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let mut buf = vec![0u8; 56 + 2 + 2 + 1];
        stream.peek_exact(&mut buf).await?;

        let password = String::from_utf8_lossy(&buf[0..56]);
        if let Err(_) = hex::decode(password.as_ref()) {
            bail!("Not trojan request.")
        }
        if !self.auth_hub.auth(&password).await? {
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
            1 => (Cmd::Connect(address.to_string()), password),
            3 => (Cmd::UdpAssociate, password),
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
