use std::io;

use super::{BoxedUdpStream, OutboundStream};
use anyhow::{Context, Result};
use futures::{SinkExt, StreamExt, TryStreamExt};
use log::info;
use tokio::{
    io::{copy_bidirectional, AsyncRead, AsyncWrite},
    net::{TcpStream, UdpSocket},
    select,
};

const UDP_BUFFER_SIZE: usize = 2048;

pub async fn accept(s: OutboundStream) -> Result<()> {
    match s {
        OutboundStream::Tcp(tcp, addr) => handle_tcp(tcp, addr).await,
        OutboundStream::Udp(udp) => handle_udp(udp).await,
    }
}

async fn handle_tcp(mut s: impl AsyncRead + AsyncWrite + Unpin, addr: String) -> Result<()> {
    let mut outbound_stream = TcpStream::connect(&addr)
        .await
        .context(format!("Unable to connect to target {}", &addr))?;

    info!("Connecting to target {}", &addr);
    copy_bidirectional(&mut s, &mut outbound_stream)
        .await
        .unwrap_or((0, 0));
    info!("Connection to target {} has closed.", &addr);

    Ok(())
}

async fn handle_udp(s: BoxedUdpStream) -> Result<()> {
    let (mut sink, mut stream) = s.split();
    let udp = UdpSocket::bind("0.0.0.0:0").await?;

    info!("UDP tunnel {} created.", &udp.local_addr()?);

    let inbound = async {
        let mut buf = [0u8; UDP_BUFFER_SIZE];
        loop {
            let (size, addr) = udp.recv_from(&mut buf).await?;
            sink.send((buf[..size].to_vec(), addr))
                .await
                .map_err(|_| Into::<io::Error>::into(io::ErrorKind::BrokenPipe))?;
        }
    };
    let outbound = async {
        loop {
            while let Some((buf, addr)) = stream.try_next().await? {
                udp.send_to(&buf, addr).await?;
            }
        }
    };

    select! {
        r = inbound => r,
        r = outbound => r,
    }
}
