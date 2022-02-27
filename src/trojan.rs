use anyhow::Result;
use bytes::{Buf, BufMut, BytesMut};
use futures::{ready, SinkExt, StreamExt};
use socks5_protocol::{sync::FromIO, Address};
use std::{
    io::{self, Write},
    net::SocketAddr,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_util::codec::{Decoder, Encoder, Framed};

use crate::common::AsyncUdp;

const UDP_MAX_SIZE: usize = 65535;
// 259 is max size of address, atype 1 + domain len 1 + domain 255 + port 2
const PREFIX_LENGTH: usize = 259 + 2 + 2;

struct UdpCodec(Option<Vec<u8>>);

impl UdpCodec {
    fn new(head: impl Into<Option<Vec<u8>>>) -> UdpCodec {
        UdpCodec(head.into())
    }
}

type UdpPacket = (Vec<u8>, Address);

impl Encoder<UdpPacket> for UdpCodec {
    type Error = io::Error;

    fn encode(&mut self, item: UdpPacket, dst: &mut BytesMut) -> Result<(), Self::Error> {
        if item.0.len() > UDP_MAX_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Frame of length {} is too large.", item.0.len()),
            ));
        }

        dst.reserve(PREFIX_LENGTH + item.0.len() + self.0.as_ref().map(|i| i.len()).unwrap_or(0));
        let mut writer = dst.writer();

        if let Some(head) = self.0.take() {
            writer.write_all(&head)?;
        }
        item.1.write_to(&mut writer).map_err(|e| e.to_io_err())?;
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

        Ok(Some((buf.into(), address)))
    }
}

pub struct TrojanUdp<S> {
    framed: Framed<S, UdpCodec>,
    flushing: bool,
}

impl<S> TrojanUdp<S>
where
    S: AsyncRead + AsyncWrite,
{
    pub fn new(stream: S, head: impl Into<Option<Vec<u8>>>) -> Self {
        Self {
            framed: Framed::new(stream, UdpCodec::new(None)),
            flushing: false,
        }
    }
}

impl<S> AsyncUdp for TrojanUdp<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_recv_from(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<SocketAddr>> {
        let (bytes, from) = match ready!(self.framed.poll_next_unpin(cx)) {
            Some(r) => r?,
            None => return Poll::Ready(Err(io::ErrorKind::UnexpectedEof.into())),
        };

        let to_copy = bytes.len().min(buf.remaining());
        buf.initialize_unfilled_to(to_copy)
            .copy_from_slice(&bytes[..to_copy]);
        buf.advance(to_copy);

        let addr = match from {
            socks5_protocol::Address::SocketAddr(addr) => addr,
            _ => return Poll::Ready(Err(io::ErrorKind::InvalidData.into())),
        };
        Poll::Ready(Ok(addr))
    }

    fn poll_send_to(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        addr: SocketAddr,
    ) -> Poll<io::Result<usize>> {
        loop {
            if self.flushing {
                ready!(self.framed.poll_flush_unpin(cx))?;
                self.flushing = false;
                return Poll::Ready(Ok(buf.len()));
            }
            ready!(self.framed.poll_ready_unpin(cx))?;
            self.framed.start_send_unpin((buf.to_vec(), addr.into()))?;
            self.flushing = true;
        }
    }
}
