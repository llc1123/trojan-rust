use super::{BoxedUdpStream, OutboundStream};
use anyhow::{anyhow, bail, Error, Result};
use futures::{SinkExt, StreamExt};
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
                Err(_) => continue,
            };
            sink.send((buf[..size].to_vec(), addr.to_string()))
                .await
                .map_err(|_| anyhow!("Broken pipe."))?;
        }
        Ok::<(), Error>(())
    };

    let outbound = async {
        while let Ok(Some(res)) = timeout(FULL_CONE_TIMEOUT, stream.next()).await {
            match res {
                Ok((buf, addr)) => {
                    if let Err(e) = udp.send_to(&buf, &addr).await {
                        warn!("Unable to send to target {}: {}", &addr, e);
                    };
                }
                Err(_) => bail!("Connection reset by peer."),
            }
        }
        Ok::<(), Error>(())
    };

    select! {
        r = inbound => r?,
        r = outbound => r?,
    };
    info!("UDP tunnel closed.");
    Ok(())
}
