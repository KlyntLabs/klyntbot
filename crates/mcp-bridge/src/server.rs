use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, warn};

use crate::protocol::{BridgeFrame, read_frame};

/// Synchronous handler invoked once per inbound frame. Must be cheap and
/// non-blocking — the connection reader awaits its return.
pub type FrameHandler = Box<dyn Fn(BridgeFrame) + Send + Sync + 'static>;

/// Owns the bound socket and the accept loop's cancellation token. Drop or
/// call `shutdown()` to stop and unlink the socket file.
pub struct BridgeServer {
    handle: BridgeServerHandle,
}

#[derive(Clone)]
pub struct BridgeServerHandle {
    shutdown: CancellationToken,
    socket_path: Arc<PathBuf>,
}

impl BridgeServer {
    /// Bind the socket, spawn the accept loop, return the running server.
    /// Removes any stale socket file at `socket_path` first.
    pub async fn start(socket_path: PathBuf, handler: FrameHandler) -> std::io::Result<Self> {
        let _ = std::fs::remove_file(&socket_path);
        if let Some(parent) = socket_path.parent() {
            // Best-effort: ensure parent dir exists (e.g. ~/.klyntbot).
            let _ = std::fs::create_dir_all(parent);
        }
        let listener = UnixListener::bind(&socket_path)?;
        let shutdown = CancellationToken::new();
        let path_arc = Arc::new(socket_path.clone());
        let handler = Arc::new(handler);
        Self::spawn_accept_loop(listener, handler, shutdown.clone());
        Ok(Self {
            handle: BridgeServerHandle {
                shutdown,
                socket_path: path_arc,
            },
        })
    }

    pub fn handle(&self) -> BridgeServerHandle {
        self.handle.clone()
    }

    /// Cancel the accept loop and unlink the socket file. Idempotent.
    pub fn shutdown(self) {
        self.handle.shutdown();
    }

    fn spawn_accept_loop(
        listener: UnixListener,
        handler: Arc<FrameHandler>,
        shutdown: CancellationToken,
    ) {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => {
                        debug!("mcp-bridge: accept loop shutdown");
                        break;
                    }
                    res = listener.accept() => match res {
                        Ok((stream, _addr)) => {
                            let h = handler.clone();
                            tokio::spawn(handle_connection(stream, h));
                        }
                        Err(e) => {
                            error!("mcp-bridge: accept error: {e}");
                            break;
                        }
                    }
                }
            }
        });
    }
}

impl BridgeServerHandle {
    pub fn shutdown(&self) {
        self.shutdown.cancel();
        let _ = std::fs::remove_file(self.socket_path.as_path());
    }
}

impl Drop for BridgeServer {
    fn drop(&mut self) {
        self.handle.shutdown();
    }
}

async fn handle_connection(mut stream: UnixStream, handler: Arc<FrameHandler>) {
    loop {
        match read_frame(&mut stream).await {
            Ok(Some(frame)) => handler(frame),
            Ok(None) => break, // clean EOF
            Err(e) => {
                warn!("mcp-bridge: frame error, dropping connection: {e}");
                break;
            }
        }
    }
}

// Keep `Path` referenced for clarity even if unused at the type level.
#[allow(dead_code)]
fn _path_marker(_p: &Path) {}
