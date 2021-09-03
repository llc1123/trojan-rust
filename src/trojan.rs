use crate::common::UdpPacket;
use anyhow::Result;
use bytes::{Buf, BufMut, BytesMut};
use socks5_protocol::{sync::FromIO, Address};
use std::{
    io::{self, Write},
    str::FromStr,
};
use tokio_util::codec::{Decoder, Encoder};

const UDP_MAX_SIZE: usize = 65535;
// 259 is max size of address, atype 1 + domain len 1 + domain 255 + port 2
const PREFIX_LENGTH: usize = 259 + 2 + 2;

pub struct UdpCodec(Option<Vec<u8>>);

impl UdpCodec {
    pub fn new(head: impl Into<Option<Vec<u8>>>) -> UdpCodec {
        UdpCodec(head.into())
    }
}

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
