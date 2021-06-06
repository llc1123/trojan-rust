use super::{tls::TlsContext, Inbound, InboundAccept, InboundRequest};
use crate::{
    auth::Auth,
    common::{AsyncStream, BoxedStream, BoxedUdpStream, UdpPacket},
    inbound::tls::TlsAccept,
    utils::{config::Trojan, peekable_stream::PeekableStream},
};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use bytes::{Buf, BufMut, BytesMut};
use fallback::FallbackAcceptor;
use log::{info, warn};
use socks5_protocol::{sync::FromIO, Address};
use std::{
    io::{self, Cursor},
    net::SocketAddr,
    str::FromStr,
    sync::Arc,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{Decoder, Encoder, Framed};

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
                    stream: Box::pin(Framed::new(stream, UdpCodec)),
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

const UDP_MAX_SIZE: usize = 65535;
// 259 is max size of address, atype 1 + domain len 1 + domain 255 + port 2
const PREFIX_LENGTH: usize = 259 + 2 + 2;

struct UdpCodec;

impl Encoder<UdpPacket> for UdpCodec {
    type Error = io::Error;

    fn encode(&mut self, item: UdpPacket, dst: &mut BytesMut) -> Result<(), Self::Error> {
        if item.0.len() > UDP_MAX_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Frame of length {} is too large.", item.0.len()),
            ));
        }

        dst.reserve(PREFIX_LENGTH + item.0.len());
        let mut writer = dst.writer();

        Address::from_str(&item.1)
            .map_err(|e| e.to_io_err())?
            .write_to(&mut writer)
            .map_err(|e| e.to_io_err())?;
        let dst = writer.into_inner();

        dst.put_u16(item.0.len() as u16);
        dst.extend_from_slice(&[0x0D, 0x0A]);
        dst.extend_from_slice(&item.0);

        Ok(())
    }
}

fn copy_2(b: &[u8]) -> [u8; 2] {
    let mut buf = [0u8; 2];
    buf.copy_from_slice(&b);
    buf
}

impl Decoder for UdpCodec {
    type Item = UdpPacket;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 2 {
            return Ok(None);
        }
        let head = copy_2(&src[0..2]);
        let addr_size = match head[0] {
            1 => 7,
            3 => 1 + head[1] as usize + 2,
            4 => 19,
            _ => return Err(io::ErrorKind::InvalidData.into()),
        };
        if src.len() < addr_size + 4 {
            return Ok(None);
        }
        let length = u16::from_be_bytes(copy_2(&src[addr_size..addr_size + 2])) as usize;
        if src.len() < addr_size + 4 + length {
            return Ok(None);
        }

        let mut reader = src.reader();
        let address = Address::read_from(&mut reader).map_err(|e| e.to_io_err())?;
        let src = reader.into_inner();

        // Length and CrLf
        src.get_u16();
        src.get_u16();

        let mut buf = vec![0u8; length as usize];

        src.copy_to_slice(&mut buf);

        Ok(Some((buf.into(), address.to_string())))
    }
}
