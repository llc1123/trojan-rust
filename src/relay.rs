use crate::auth::Auth;
use crate::common::UdpStream;
use crate::inbound::{Inbound, InboundAccept, InboundRequest};
use crate::outbound::Outbound;
use crate::utils::count_stream::CountStream;
use anyhow::{anyhow, bail, Context, Error, Result};
use futures::{future::try_select, SinkExt, StreamExt, TryStreamExt};
use log::{error, info, warn};
use std::{io, sync::Arc, time::Duration};
use tokio::{
    io::{copy, split, AsyncRead, AsyncWrite, AsyncWriteExt},
    net::TcpListener,
    pin, select,
    sync::mpsc::{unbounded_channel, UnboundedReceiver},
    time::timeout,
};
use tokio_io_timeout::TimeoutStream;
use tokio_stream::wrappers::UnboundedReceiverStream;

const FULL_CONE_TIMEOUT: Duration = Duration::from_secs(30);

pub struct Relay<I, O> {
    listener: TcpListener,
    inbound: Arc<I>,
    outbound: Arc<O>,
    tcp_nodelay: bool,
    pub tcp_timeout: Option<Duration>,
}

/// Connect two `TcpStream`. Unlike `copy_bidirectional`, it closes the other side once one side is done.
pub async fn connect_tcp(
    t1: impl AsyncRead + AsyncWrite,
    t2: impl AsyncRead + AsyncWrite,
) -> io::Result<()> {
    let (mut read_1, mut write_1) = split(t1);
    let (mut read_2, mut write_2) = split(t2);

    let fut1 = async {
        let r = copy(&mut read_1, &mut write_2).await;
        write_2.shutdown().await?;
        r
    };
    let fut2 = async {
        let r = copy(&mut read_2, &mut write_1).await;
        write_1.shutdown().await?;
        r
    };

    pin!(fut1, fut2);

    match try_select(fut1, fut2).await {
        Ok(_) => {}
        Err(e) => return Err(e.factor_first().0),
    };

    Ok(())
}

impl<I, O> Relay<I, O>
where
    I: Inbound + 'static,
    O: Outbound + 'static,
{
    async fn process(
        outbound: Arc<O>,
        inbound_accept: InboundAccept<I::Metadata, I::TcpStream, I::UdpSocket>,
    ) -> Result<()> {
        match inbound_accept.request {
            InboundRequest::TcpConnect { addr, stream } => {
                let target = outbound.tcp_connect(&addr).await?;
                pin!(target, stream);
                connect_tcp(&mut stream, &mut target)
                    .await
                    .map_err(|e| anyhow!("Connection reset by peer. {:?}", e))?;
            }
            InboundRequest::UdpBind { addr, stream } => {
                let target = outbound.udp_bind(&addr).await?;
                pin!(target, stream);
                info!("UDP tunnel created.");
                handle_udp(&mut stream, &mut target).await?;
                stream.close().await?;
                target.close().await?;
            }
        }
        Ok(())
    }
}

struct PacketStat {
    password: String,
    upload: u64,
    download: u64,
}

impl PacketStat {
    // conn_id, upload, download
    fn new(password: String, upload: u64, download: u64) -> Self {
        PacketStat {
            password,
            upload,
            download,
        }
    }
}

impl<I, O> Relay<I, O> {
    pub fn new(listener: TcpListener, inbound: I, outbound: O, tcp_nodelay: bool) -> Self {
        Relay {
            listener,
            inbound: Arc::new(inbound),
            outbound: Arc::new(outbound),
            tcp_nodelay,
            tcp_timeout: None,
        }
    }
}

impl<I, O> Relay<I, O>
where
    I: Inbound<Metadata = String> + 'static,
    O: Outbound + 'static,
{
    pub async fn serve_trojan(&self, auth: Arc<dyn Auth>) -> Result<()> {
        let (tx, rx) = unbounded_channel::<PacketStat>();
        tokio::spawn(stat(auth, rx));
        loop {
            let (stream, addr) = self.listener.accept().await?;
            let local_addr = stream.local_addr()?;
            info!("Inbound connection from {}", addr);
            stream
                .set_nodelay(self.tcp_nodelay)
                .context("Set TCP_NODELAY failed")?;

            let (stream, sender) = CountStream::new2(stream);
            let mut stream = TimeoutStream::new(stream);
            stream.set_read_timeout(self.tcp_timeout);

            let inbound = self.inbound.clone();
            let outbound = self.outbound.clone();
            let t = tx.clone();
            tokio::spawn(async move {
                let inbound_accept = inbound.accept(Box::pin(stream), addr, local_addr).await?;
                if let Some(accept) = inbound_accept {
                    // Here we got trojan password

                    let password = accept.metadata.clone();
                    sender
                        .send(Box::new(move |read: usize, write: usize| {
                            t.send(PacketStat::new(password, read as u64, write as u64))
                                .ok();
                        }))
                        .ok();
                    if let Err(e) = Self::process(outbound, accept).await {
                        warn!("Relay error: {:?}", e)
                    }
                }
                Ok(()) as Result<()>
            });
        }
    }
}

impl<I, O> Relay<I, O>
where
    I: Inbound<Metadata = ()> + 'static,
    O: Outbound + 'static,
{
    pub async fn serve_socks5(&self) -> Result<()> {
        loop {
            let (stream, addr) = self.listener.accept().await?;
            let local_addr = stream.local_addr()?;
            info!("Inbound connection from {}", addr);
            stream
                .set_nodelay(self.tcp_nodelay)
                .context("Set TCP_NODELAY failed")?;

            let mut stream = TimeoutStream::new(stream);
            stream.set_read_timeout(self.tcp_timeout);

            let inbound = self.inbound.clone();
            let outbound = self.outbound.clone();
            tokio::spawn(async move {
                let inbound_accept = inbound.accept(Box::pin(stream), addr, local_addr).await?;
                if let Some(accept) = inbound_accept {
                    if let Err(e) = Self::process(outbound, accept).await {
                        warn!("Relay error: {:?}", e)
                    }
                }
                Ok(()) as Result<()>
            });
        }
    }
}

async fn stat(auth: Arc<dyn Auth>, rx: UnboundedReceiver<PacketStat>) {
    let stream = UnboundedReceiverStream::new(rx);
    stream
        .for_each_concurrent(10, |i| {
            let auth = auth.clone();
            async move {
                if let Err(e) = auth.stat(&i.password, i.upload, i.download).await {
                    error!("Failed to stat: {:?}", e);
                }
            }
        })
        .await;
}

async fn handle_udp(incoming: impl UdpStream, target: impl UdpStream) -> Result<()> {
    let (mut at, mut ar) = incoming.split();
    let (mut bt, mut br) = target.split();

    let inbound = async {
        loop {
            let (buf, addr) = match br.try_next().await {
                Ok(Some(r)) => r,
                _ => continue,
            };
            at.send((buf, addr))
                .await
                .map_err(|_| anyhow!("Broken pipe."))?;
        }
        #[allow(unreachable_code)]
        Ok::<(), Error>(())
    };

    let outbound = async {
        while let Ok(Some(res)) = timeout(FULL_CONE_TIMEOUT, ar.next()).await {
            match res {
                Ok((buf, addr)) => {
                    if let Err(e) = bt.send((buf, addr.clone())).await {
                        warn!("Unable to send to target {}: {}", addr, e);
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
