use std::io::{self, ErrorKind};

use crate::common::UdpPacket;

use super::{BoxedUdpStream, Outbound};
use anyhow::Result;
use async_trait::async_trait;
use bytes::{BufMut, Bytes, BytesMut};
use futures::{SinkExt, StreamExt};
use log::info;
use tokio::net::{lookup_host, TcpStream, UdpSocket};
use tokio_util::{
    codec::{Decoder, Encoder},
    udp::UdpFramed,
};

pub struct BytesCodec(());

impl BytesCodec {
    // Creates a new `BytesCodec` for shipping around raw bytes.
    pub fn new() -> BytesCodec {
        BytesCodec(())
    }
}

impl Decoder for BytesCodec {
    type Item = Bytes;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Bytes>, io::Error> {
        if !buf.is_empty() {
            let len = buf.len();
            Ok(Some(buf.split_to(len).freeze()))
        } else {
            Ok(None)
        }
    }
}

impl Encoder<Bytes> for BytesCodec {
    type Error = io::Error;

    fn encode(&mut self, data: Bytes, buf: &mut BytesMut) -> Result<(), io::Error> {
        buf.reserve(data.len());
        buf.put(data);
        Ok(())
    }
}

pub struct DirectOutbound;

impl DirectOutbound {
    pub fn new() -> Self {
        DirectOutbound
    }
}

#[async_trait]
impl Outbound for DirectOutbound {
    type TcpStream = TcpStream;
    type UdpSocket = BoxedUdpStream;

    async fn tcp_connect(&self, address: &str) -> io::Result<Self::TcpStream> {
        info!("Connecting to target {}", address);
        TcpStream::connect(address).await
    }

    async fn udp_bind(&self, address: &str) -> io::Result<Self::UdpSocket> {
        let udp = UdpSocket::bind(address).await?;
        let stream = UdpFramed::new(udp, BytesCodec::new())
            .map(|r| r.map(|(a, b)| (a, b.to_string())))
            .with(|(buf, addr): UdpPacket| async move {
                let addr = lookup_host(addr)
                    .await?
                    .next()
                    .ok_or(io::Error::from(ErrorKind::AddrNotAvailable))?;
                Ok((buf, addr)) as io::Result<_>
            });
        Ok(Box::pin(stream))
    }
}
