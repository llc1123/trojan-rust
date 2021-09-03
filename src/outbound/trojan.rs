use std::io::{self, Cursor};

use crate::{trojan::UdpCodec, utils::config::client::TrojanServer};

use self::tcp::TrojanTcp;
use super::{BoxedUdpStream, Outbound};
use anyhow::Result;
use async_trait::async_trait;
use log::info;
use sha2::{Digest, Sha224};
use socks5_protocol::{sync::FromIO, Address};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

mod tcp;
mod tls;

pub struct TrojanOutbound {
    connector: tls::TlsConnector,
    config: TrojanServer,
    password: Box<[u8]>,
}

impl TrojanOutbound {
    pub fn new(config: TrojanServer) -> Result<Self> {
        let connector = tls::TlsConnector::new(config.sni.clone(), config.skip_cert_verify)?;
        let password = hex::encode(Sha224::digest(config.password.as_bytes()));

        Ok(TrojanOutbound {
            connector,
            config,
            password: password.as_bytes().into(),
        })
    }
}

impl TrojanOutbound {
    // cmd 1 for Connect, 3 for Udp associate
    fn make_head(&self, cmd: u8, addr: Address) -> io::Result<Vec<u8>> {
        use std::io::Write;

        let head = Vec::<u8>::new();
        let mut writer = Cursor::new(head);

        Write::write_all(&mut writer, &self.password)?;
        Write::write_all(&mut writer, b"\r\n")?;
        // Connect
        Write::write_all(&mut writer, &[cmd])?;
        addr.write_to(&mut writer).map_err(|e| e.to_io_err())?;
        Write::write_all(&mut writer, b"\r\n")?;

        Ok(writer.into_inner())
    }
}

#[async_trait]
impl Outbound for TrojanOutbound {
    type TcpStream = TrojanTcp;
    type UdpSocket = BoxedUdpStream;

    async fn tcp_connect(&self, address: &str) -> io::Result<Self::TcpStream> {
        info!("Connecting to target {}", address);
        let inner = TcpStream::connect(&self.config.server).await?;

        let stream = self.connector.connect(inner).await?;
        let head = self.make_head(
            1,
            address
                .parse()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?,
        )?;

        let tcp = TrojanTcp::new(stream, head);

        Ok(tcp)
    }

    async fn udp_bind(&self, address: &str) -> io::Result<Self::UdpSocket> {
        info!("Connecting to target {}", address);
        let inner = TcpStream::connect(&self.config.server).await?;

        let stream = self.connector.connect(inner).await?;
        let head = self.make_head(
            3,
            address
                .parse()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?,
        )?;

        Ok(Box::pin(Framed::new(stream, UdpCodec::new(head))))
    }
}
