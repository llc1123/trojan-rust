use crate::auth::Auth;
use crate::common::UdpStream;
use crate::inbound::{Inbound, InboundAccept, InboundRequest};
use crate::outbound::Outbound;
use crate::utils::count_stream::CountStream;
use anyhow::{anyhow, bail, Context, Error, Result};
use futures::{SinkExt, StreamExt, TryStreamExt};
use log::error;
use log::{info, warn};
use std::{sync::Arc, time::Duration};
use tokio::net::TcpListener;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
use tokio::{io::copy_bidirectional, pin, select, time::timeout};
use tokio_stream::wrappers::UnboundedReceiverStream;

const FULL_CONE_TIMEOUT: Duration = Duration::from_secs(30);

pub struct Relay<I, O> {
    listener: TcpListener,
    inbound: Arc<I>,
    outbound: Arc<O>,
    tcp_nodelay: bool,
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
                pin!(target);
                pin!(stream);
                copy_bidirectional(&mut stream, &mut target)
                    .await
                    .map_err(|_| anyhow!("Connection reset by peer."))?;
            }
            InboundRequest::UdpBind { addr, stream } => {
                let target = outbound.udp_bind(&addr).await?;
                info!("UDP tunnel created.");
                handle_udp(stream, target).await?;
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

impl<I, O> Relay<I, O>
where
    I: Inbound<Metadata = String> + 'static,
    O: Outbound + 'static,
{
    pub fn new(listener: TcpListener, inbound: I, outbound: O, tcp_nodelay: bool) -> Self {
        Relay {
            listener,
            inbound: Arc::new(inbound),
            outbound: Arc::new(outbound),
            tcp_nodelay,
        }
    }
    pub async fn serve_trojan(&self, auth: Arc<dyn Auth>) -> Result<()> {
        let (tx, rx) = unbounded_channel::<PacketStat>();
        tokio::spawn(stat(auth, rx));
        loop {
            let (stream, addr) = self.listener.accept().await?;
            info!("Inbound connection from {}", addr);
            stream
                .set_nodelay(self.tcp_nodelay)
                .context("Set TCP_NODELAY failed")?;

            let (stream, sender) = CountStream::new2(stream);

            let inbound = self.inbound.clone();
            let outbound = self.outbound.clone();
            let t = tx.clone();
            tokio::spawn(async move {
                let inbound_accept = inbound.accept(stream, addr).await?;
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
