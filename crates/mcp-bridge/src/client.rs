use std::path::PathBuf;
use std::time::Duration;
use tokio::net::UnixStream;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::trace;

use crate::protocol::{write_frame, BridgeFrame};

const CONNECT_TIMEOUT: Duration = Duration::from_millis(200);
const WRITE_TIMEOUT: Duration = Duration::from_millis(200);

/// Write-only async client for the bridge socket.
///
/// `send` is non-blocking and infallible from the caller's perspective:
/// frames are pushed onto an unbounded mpsc and a background task drains it
/// to the socket. If the desktop process isn't running, frames are dropped
/// silently after a connect attempt times out (200 ms).
#[derive(Clone)]
pub struct BridgeClient {
    tx: mpsc::UnboundedSender<BridgeFrame>,
}

impl BridgeClient {
    pub fn new(socket_path: PathBuf) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(writer_loop(socket_path, rx));
        Self { tx }
    }

    /// Enqueue a frame for delivery. Never blocks. Drops silently if the
    /// internal channel is closed (writer task panicked — should not happen).
    pub fn send(&self, frame: BridgeFrame) {
        if let Err(e) = self.tx.send(frame) {
            trace!("mcp-bridge: client send dropped (channel closed): {e}");
        }
    }
}

/// Drains the channel forever, lazy-connecting on demand. Each iteration:
/// 1. Wait for the next frame (returns when channel closes → exit).
/// 2. Try to connect (200 ms timeout). On failure, drop the frame and loop.
/// 3. Try to write. On any error, drop the connection and loop.
async fn writer_loop(socket_path: PathBuf, mut rx: mpsc::UnboundedReceiver<BridgeFrame>) {
    while let Some(frame) = rx.recv().await {
        if let Err(e) = send_one(&socket_path, &frame).await {
            trace!(error = %e, path = ?socket_path, "mcp-bridge: send dropped");
        }
    }
    trace!("mcp-bridge: writer loop exiting (client dropped)");
}

async fn send_one(
    socket_path: &PathBuf,
    frame: &BridgeFrame,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut stream = timeout(CONNECT_TIMEOUT, UnixStream::connect(socket_path))
        .await
        .map_err(|_| "connect timeout")??;
    timeout(WRITE_TIMEOUT, async {
        write_frame(&mut stream, frame).await?;
        // Mirror `coding-ingest`: shutdown signals end-of-frame to the server.
        use tokio::io::AsyncWriteExt;
        stream.shutdown().await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    })
    .await
    .map_err(|_| "write timeout")??;
    Ok(())
}
