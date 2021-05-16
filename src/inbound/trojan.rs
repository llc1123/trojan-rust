use std::io::{self, Cursor};

use anyhow::{bail, Result};
use bytes::{Buf, BufMut, BytesMut};
use log::info;
use socks5_protocol::{sync::FromIO, Address, Command, CommandRequest};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{Decoder, Encoder, Framed};

const CMD: usize = 58;

pub enum Cmd {
    Connect(String),
    UdpAssociate,
}

use crate::{
    auth::{Auth, AuthHub},
    outbound::{OutboundStream, UdpPacket},
    utils::peekable_stream::PeekableStream,
};

pub struct TrojanAcceptor {
    auth_hub: AuthHub,
}

impl TrojanAcceptor {
    pub fn new(auth_hub: AuthHub) -> Result<TrojanAcceptor> {
        Ok(TrojanAcceptor { auth_hub })
    }

    pub async fn accept<IO>(&self, stream: IO) -> Result<OutboundStream, PeekableStream<IO>>
    where
        IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let mut stream = PeekableStream::new(stream);
        match self.inner_accept(&mut stream).await {
            Ok(Cmd::Connect(addr)) => Ok(OutboundStream::Tcp(Box::pin(stream), addr)),
            Ok(Cmd::UdpAssociate) => {
                Ok(OutboundStream::Udp(Box::pin(Framed::new(stream, UdpCodec))))
            }
            Err(e) => {
                info!("Redirect to fallback: {:?}", e);
                return Err(stream);
            }
        }
    }

    async fn inner_accept<IO>(&self, stream: &mut PeekableStream<IO>) -> Result<Cmd>
    where
        IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let mut buf = vec![0u8; 56 + 2 + 2];
        stream.peek_exact(&mut buf).await?;

        let password = String::from_utf8_lossy(&buf[0..56]);
        if let Err(_) = hex::decode(password.as_ref()) {
            bail!("Not trojan request.")
        }
        if !self.auth_hub.auth(&password).await? {
            bail!("{}", &password)
        }

        let mut reader = Cursor::new(buf);
        // skip password and CRLF
        reader.advance(CMD);

        let req = CommandRequest::read_from(&mut reader)?;
        let end = reader.position() + 2;
        stream.drain(end as usize).await?;

        match req.command {
            Command::Connect => Ok(Cmd::Connect(req.address.to_string())),
            Command::UdpAssociate => Ok(Cmd::UdpAssociate),
            _ => bail!("Unknown command."),
        }
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

        Address::from(item.1)
            .write_to(&mut writer)
            .map_err(|e| e.to_io_err())?;
        let dst = writer.into_inner();

        dst.put_u16(item.0.len() as u16);
        dst.extend_from_slice(&[0x0D, 0x0A]);
        dst.extend_from_slice(&item.0);

        Ok(())
    }
}

impl Decoder for UdpCodec {
    type Item = UdpPacket;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let mut reader = src.reader();
        let result = Address::read_from(&mut reader);
        let src = reader.into_inner();

        let address = match result {
            Ok(a) => a,
            Err(socks5_protocol::Error::Io(e)) if e.kind() == io::ErrorKind::UnexpectedEof => {
                return Ok(None);
            }
            Err(e) => return Err(e.to_io_err()),
        };
        if src.remaining() < 4 {
            return Ok(None);
        }

        let length = src.get_u16();
        // CrLf
        src.get_u16();

        let mut buf = vec![0u8; length as usize];

        src.copy_to_slice(&mut buf);

        Ok(Some((
            buf,
            address.to_socket_addr().map_err(|e| e.to_io_err())?,
        )))
    }
}
