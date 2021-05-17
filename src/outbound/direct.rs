use super::{BoxedUdpStream, OutboundStream};
use anyhow::{anyhow, bail, Result};
use futures::{SinkExt, StreamExt, TryStreamExt};
use log::{info, warn};
use tokio::{
    io::{copy_bidirectional, AsyncRead, AsyncWrite},
    net::{TcpStream, UdpSocket},
    select,
    time::{timeout, Duration},
};

const UDP_BUFFER_SIZE: usize = 2048;
const FULL_CONE_TIMEOUT: Duration = Duration::from_secs(30);

pub async fn accept(s: OutboundStream) -> Result<()> {
    let r = match s {
        OutboundStream::Tcp(tcp, addr) => handle_tcp(tcp, addr).await,
        OutboundStream::Udp(udp) => handle_udp(udp).await,
    };

    Ok(r.unwrap_or_else(|err| {
        warn!("{}", err);
    }))
}

async fn handle_tcp(mut s: impl AsyncRead + AsyncWrite + Unpin, addr: String) -> Result<()> {
    info!("Connecting to target {}", &addr);

    let mut outbound_stream = TcpStream::connect(&addr)
        .await
        .map_err(|err| anyhow!("Unable to connect to target {}: {}", &addr, err))?;

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
        loop {
            let (size, addr) = match udp.recv_from(&mut buf).await {
                Ok(r) => r,
                // ignore recv_from error
                Err(_e) => continue,
            };
            sink.send((buf[..size].to_vec(), addr))
                .await
                .map_err(|_| anyhow!("Broken pipe."))?;
        }
    };

    let outbound = async {
        loop {
            match timeout(FULL_CONE_TIMEOUT, stream.try_next()).await {
                Ok(Ok(Some((buf, addr)))) => {
                    if let Err(e) = udp.send_to(&buf, addr).await {
                        warn!("Unable to send to target {}: {}", &addr, e);
                    };
                    continue;
                }
                Ok(Ok(None)) => continue,
                Ok(Err(_)) => bail!("Broken pipe."),
                Err(_) => break,
            };
        }
        info!("UDP tunnel closed.");
        Ok(())
    };

    select! {
        r = inbound => r,
        r = outbound => r,
    }
}
