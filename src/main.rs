use std::net::SocketAddr;

use anyhow::Result;
use clap::Parser;
use log::warn;
use once_cell::sync::OnceCell;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpStream,
    },
};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

static ARGS: OnceCell<Args> = OnceCell::new();

/// Simple program to perform TCP port forwarding
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Socket to listen
    #[arg(short, long)]
    listen: SocketAddr,

    /// Remote address
    #[arg(short, long)]
    remote: SocketAddr,
}

async fn forward(mut src_rx: OwnedReadHalf, mut dst_tx: OwnedWriteHalf) -> Result<()> {
    let mut buffer = vec![0u8; 8192];
    loop {
        let n = src_rx.read(&mut buffer).await?;
        dst_tx.write_all(&buffer[..n]).await?;
    }
}

async fn backward(mut dst_rx: OwnedReadHalf, mut src_tx: OwnedWriteHalf) -> Result<()> {
    let mut buffer = vec![0u8; 8192];
    loop {
        let n = dst_rx.read(&mut buffer).await?;
        src_tx.write_all(&buffer[..n]).await?;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry().with(fmt::layer()).init();

    let args = Args::parse();
    ARGS.get_or_init(|| args);

    let listener = TcpListener::bind(ARGS.wait().listen).await?;

    loop {
        let (src_stream, _) = listener.accept().await?;
        let (src_rx, src_tx) = src_stream.into_split();

        let dst_stream = TcpStream::connect(ARGS.wait().remote).await?;
        let (dst_rx, dst_tx) = dst_stream.into_split();

        let forward = tokio::spawn(async move {
            if let Err(e) = forward(src_rx, dst_tx).await {
                warn!("Connection failed: {e}");
            }
        });

        let backward = tokio::spawn(async move {
            if let Err(e) = backward(dst_rx, src_tx).await {
                warn!("Connection failed: {e}");
            }
        });

        tokio::select! {
            _ = forward => {},
            _ = backward => {},
        }
    }
}
