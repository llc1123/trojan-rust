use super::{BoxedUdpStream, OutboundStream};
use anyhow::{anyhow, Result};
use futures::{SinkExt, StreamExt, TryStreamExt};
use log::{info, warn};
use std::io;
use tokio::{
    io::{copy_bidirectional, AsyncRead, AsyncWrite},
    net::{TcpStream, UdpSocket},
    pin, select,
    time::{sleep, Duration, Instant},
};

const UDP_BUFFER_SIZE: usize = 2048;
const FULL_CONE_TIMEOUT: u64 = 30;

pub async fn accept(s: OutboundStream) -> Result<()> {
    let r = match s {
        OutboundStream::Tcp(tcp, addr) => handle_tcp(tcp, addr).await,
        OutboundStream::Udp(udp) => handle_udp(udp).await,
    };

    Ok(r.unwrap_or_else(|op| {
        warn!("{}", op);
    }))
}

async fn handle_tcp(mut s: impl AsyncRead + AsyncWrite + Unpin, addr: String) -> Result<()> {
    info!("Connecting to target {}", &addr);

    let mut outbound_stream = TcpStream::connect(&addr)
        .await
        .map_err(|op| anyhow!("Unable to connect to target {}: {}", &addr, op))?;

    copy_bidirectional(&mut s, &mut outbound_stream)
        .await
        .map_err(|_| anyhow!("Connection reset by peer."))?;

    info!("Connection to target {} has closed.", &addr);

    Ok(())
}

async fn handle_udp(s: BoxedUdpStream) -> Result<()> {
    let (mut sink, mut stream) = s.split();
    let udp = UdpSocket::bind("0.0.0.0:0").await?;

    info!("UDP tunnel {} created.", &udp.local_addr()?);

    let inbound = async {
        let mut buf = [0u8; UDP_BUFFER_SIZE];
        let (size, addr) = udp.recv_from(&mut buf).await?;
        sink.send((buf[..size].to_vec(), addr))
            .await
            .map_err(|_| Into::<io::Error>::into(io::ErrorKind::BrokenPipe))
    };
    pin!(inbound);

    let outbound = async {
        while let Some((buf, addr)) = stream.try_next().await? {
            udp.send_to(&buf, addr).await?;
        }
        Ok::<(), anyhow::Error>(())
    };
    pin!(outbound);

    let timeout = sleep(Duration::from_secs(FULL_CONE_TIMEOUT));
    pin!(timeout);

    loop {
        select! {
            r = &mut inbound => {
                timeout.as_mut().reset(Instant::now() + Duration::from_secs(FULL_CONE_TIMEOUT));
                r.unwrap_or_else(|err| warn!("{}", err));
            },
            r = &mut outbound => {
                timeout.as_mut().reset(Instant::now() + Duration::from_secs(FULL_CONE_TIMEOUT));
                r.unwrap_or_else(|err| warn!("{}", err));
            },
            _ = &mut timeout => {
                info!("UDP tunnel closed.");
                break;
            }
        };
    }

    Ok(())
}
