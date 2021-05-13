use super::{OutboundStream, UdpStream};
use anyhow::{Context, Result};
use futures::{SinkExt, StreamExt};
use log::info;
use std::ops::DerefMut;
use tokio::{io::copy_bidirectional, net::TcpStream};

pub async fn accept(s: OutboundStream) -> Result<()> {
    match s {
        OutboundStream::Tcp(tcp, addr) => handle_tcp(tcp, addr).await,
        OutboundStream::Udp(udp) => handle_udp(udp).await,
    }
}

async fn handle_tcp(mut s: TcpStream, addr: String) -> Result<()> {
    let mut outbound_stream = TcpStream::connect(&addr)
        .await
        .context(format!("Unable to connect to target {}", &addr))?;

    info!("Connecting to target {}", &addr);
    copy_bidirectional(&mut s, &mut outbound_stream).await?;
    info!("Connection to target {} has closed.", &addr);

    Ok(())
}

async fn handle_udp(s: Box<dyn UdpStream>) -> Result<()> {
    let (sink, stream) = s.deref_mut().split();

    Ok(())
}
