//! Desktop-embedded ingestion daemon — owns the Unix-socket lifecycle, the
//! file-buffer drainer, and the `desktop.lock` heartbeat.

use crate::event::AgentEvent;
use crate::store::IngestEventLogRepo;
use crate::transport::FileBufferFallback;
use common::{KlyntbotError, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;
use tokio::sync::oneshot;

/// Configuration for the ingestion daemon.
#[derive(Clone)]
pub struct IngestDaemonConfig {
    /// Where the Unix socket is bound.
    pub socket_path: PathBuf,
    /// Where the cold-path file buffer lives.
    pub buffer_path: PathBuf,
    /// Desktop liveness touch-file path.
    pub lock_path: PathBuf,
    /// Repo that receives decoded events.
    pub repo: Arc<IngestEventLogRepo>,
    /// Optional real-time event forwarder — e.g. to the coding-memory Distiller.
    pub event_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::event::AgentEvent>>,
    /// Optional request/response handler for `op` frames (e.g. recall context).
    pub op_handler: Option<Arc<dyn OpHandler>>,
}

impl std::fmt::Debug for IngestDaemonConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IngestDaemonConfig")
            .field("socket_path", &self.socket_path)
            .field("buffer_path", &self.buffer_path)
            .field("lock_path", &self.lock_path)
            .field("op_handler", &self.op_handler.is_some())
            .finish_non_exhaustive()
    }
}

/// Handle returned by [`spawn`]; used to shutdown cleanly.
#[derive(Debug)]
pub struct IngestDaemonHandle {
    shutdown_tx: Option<oneshot::Sender<()>>,
    accept_task: tokio::task::JoinHandle<()>,
    heartbeat_task: tokio::task::JoinHandle<()>,
}

impl IngestDaemonHandle {
    /// Signal shutdown and wait for the accept loop + heartbeat to exit.
    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        let _ = self.accept_task.await;
        self.heartbeat_task.abort();
        let _ = self.heartbeat_task.await;
    }
}

/// Bind the socket, spawn accept loop + heartbeat + buffer-drain.
pub async fn spawn(cfg: IngestDaemonConfig) -> Result<IngestDaemonHandle> {
    if cfg.socket_path.exists() {
        let _ = tokio::fs::remove_file(&cfg.socket_path).await;
    }
    if let Some(parent) = cfg.socket_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let listener = UnixListener::bind(&cfg.socket_path)
        .map_err(|e| KlyntbotError::Storage(format!("bind {}: {e}", cfg.socket_path.display())))?;

    // Drain any buffered events from a prior desktop-off window.
    if cfg.buffer_path.exists() {
        let drained = drain_buffer(&cfg.buffer_path, cfg.repo.as_ref()).await?;
        tracing::info!(drained, "ingest buffer drained on startup");
    }
    // Prune old rotated siblings.
    let buf = FileBufferFallback::new(cfg.buffer_path.clone());
    let _ = buf.prune_older().await;

    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
    let repo = cfg.repo.clone();

    let event_tx = cfg.event_tx.clone();
    let op_handler = cfg.op_handler.clone();
    let accept_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => break,
                accept = listener.accept() => {
                    match accept {
                        Ok((stream, _)) => {
                            let repo = repo.clone();
                            let event_tx = event_tx.clone();
                            let op_handler = op_handler.clone();
                            tokio::spawn(async move {
                                if let Err(e) = handle_connection(stream, repo, event_tx, op_handler).await {
                                    tracing::warn!(error = %e, "ingest handler failed");
                                }
                            });
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "accept failed");
                        }
                    }
                }
            }
        }
    });

    // Heartbeat task — refreshes `desktop.lock` mtime every 30s.
    let lock = cfg.lock_path.clone();
    let heartbeat_task = tokio::spawn(async move {
        loop {
            if let Err(e) = crate::desktop_lock::write_heartbeat(&lock).await {
                tracing::warn!(error = %e, "heartbeat write failed");
            }
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    });

    Ok(IngestDaemonHandle {
        shutdown_tx: Some(shutdown_tx),
        accept_task,
        heartbeat_task,
    })
}

const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;

/// Handler for JSON `op` request/response frames sent over the ingest socket.
#[async_trait::async_trait]
pub trait OpHandler: Send + Sync {
    /// Handle an `op` payload and return a JSON response.
    async fn handle(&self, payload: serde_json::Value) -> common::Result<serde_json::Value>;
}

async fn handle_connection(
    mut stream: tokio::net::UnixStream,
    repo: Arc<IngestEventLogRepo>,
    event_tx: Option<tokio::sync::mpsc::UnboundedSender<AgentEvent>>,
    op_handler: Option<Arc<dyn OpHandler>>,
) -> Result<()> {
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("read len: {e}")))?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_PAYLOAD_BYTES {
        return Err(KlyntbotError::Storage(format!("payload too large: {len}")));
    }
    let mut body = vec![0u8; len];
    stream
        .read_exact(&mut body)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("read body: {e}")))?;

    // Try to parse as a generic JSON value first to inspect for `op` field.
    let json_val: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| KlyntbotError::Storage(format!("decode json: {e}")))?;

    if json_val.get("op").is_some() {
        // Route through op_handler if available.
        if let Some(handler) = op_handler {
            let resp = handler
                .handle(json_val)
                .await
                .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}));
            let resp_bytes = serde_json::to_vec(&resp)
                .map_err(|e| KlyntbotError::Storage(format!("encode resp: {e}")))?;
            let resp_len = (resp_bytes.len() as u32).to_le_bytes();
            stream
                .write_all(&resp_len)
                .await
                .map_err(|e| KlyntbotError::Storage(format!("write resp len: {e}")))?;
            stream
                .write_all(&resp_bytes)
                .await
                .map_err(|e| KlyntbotError::Storage(format!("write resp body: {e}")))?;
        }
        return Ok(());
    }

    // Otherwise treat as AgentEvent (fire-and-forget).
    let event: AgentEvent = serde_json::from_value(json_val)
        .map_err(|e| KlyntbotError::Storage(format!("decode event: {e}")))?;
    repo.insert(&event).await?;
    if let Some(tx) = event_tx {
        let _ = tx.send(event);
    }
    Ok(())
}

/// Read the JSONL buffer line-by-line, insert each event, then archive the file.
pub async fn drain_buffer(path: &std::path::Path, repo: &IngestEventLogRepo) -> Result<usize> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    let f = tokio::fs::File::open(path)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("open buffer: {e}")))?;
    let mut lines = BufReader::new(f).lines();
    let mut n = 0usize;
    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|e| KlyntbotError::Storage(format!("read buffer: {e}")))?
    {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<AgentEvent>(&line) {
            Ok(evt) => {
                if let Err(e) = repo.insert(&evt).await {
                    tracing::warn!(error = %e, "drain insert failed");
                } else {
                    n += 1;
                }
            }
            Err(e) => tracing::warn!(error = %e, "drain: bad line skipped"),
        }
    }
    // Archive the drained buffer.
    let archive = path.with_extension(format!(
        "jsonl.done.{}",
        jiff::Timestamp::now().as_millisecond()
    ));
    tokio::fs::rename(path, &archive)
        .await
        .map_err(|e| KlyntbotError::Storage(format!("archive buffer: {e}")))?;
    Ok(n)
}
