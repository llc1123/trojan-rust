use std::{future::ready, io, sync::Arc};

use crate::{common::UdpPacket, utils::acl::ACL};

use super::{BoxedUdpStream, Outbound};
use anyhow::Result;
use async_trait::async_trait;
use bytes::{BufMut, Bytes, BytesMut};
use futures::{stream, FutureExt, SinkExt, StreamExt};
use log::{info, warn};
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

pub struct DirectOutbound {
    acl: Arc<ACL>,
}

impl DirectOutbound {
    pub fn new(acl: ACL) -> Self {
        DirectOutbound { acl: Arc::new(acl) }
    }
}

#[async_trait]
impl Outbound for DirectOutbound {
    type TcpStream = TcpStream;
    type UdpSocket = BoxedUdpStream;

    async fn tcp_connect(&self, address: &str) -> io::Result<Self::TcpStream> {
        info!("Connecting to target {}", address);

        let addrs = lookup_host(address).await?.filter(|addr| {
            if self.acl.has_match(addr) {
                warn!("ACL blocked: {}", &addr);
                return false;
            }
            return true;
        });

        let mut last_err = None;

        for addr in addrs {
            match TcpStream::connect(addr).await {
                Ok(stream) => return Ok(stream),
                Err(e) => last_err = Some(e),
            }
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "could not resolve to any address",
            )
        }))
    }

    async fn udp_bind(&self, address: &str) -> io::Result<Self::UdpSocket> {
        let udp = UdpSocket::bind(address).await?;
        let acl = self.acl.clone();
        let stream = UdpFramed::new(udp, BytesCodec::new())
            .map(|r| r.map(|(a, b)| (a, b.to_string())))
            .with_flat_map(move |(buf, addr): UdpPacket| {
                let acl = acl.clone();
                stream::once(lookup_host(addr).map(move |r| {
                    r.map(|mut i| i.next())
                        .ok()
                        .flatten()
                        .and_then(|i| (!acl.has_match(&i)).then(|| Ok((buf, i))))
                }))
                .filter_map(|r| ready(r))
            });
        Ok(Box::pin(stream))
    }
}
