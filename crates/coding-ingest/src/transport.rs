//! Transport stubs — Unix socket hot path + file-buffer cold path.
//!
//! Phase 1 defines the trait and struct shells so `daemon.rs` and the
//! `klyntbot-hook` binary have types to reference. Actual IO lands in
//! Phase 2.

use crate::AgentEvent;
use async_trait::async_trait;
use common::{KlyntbotError, Result};
use std::path::PathBuf;

/// Ingest channel — the hook writer's view of "send one event."
///
/// Implementations are expected to be async-safe (used from tokio context)
/// but may block briefly on filesystem operations.
#[async_trait]
pub trait IngestSocket: Send + Sync {
    /// Write one event.
    async fn send(&self, event: &AgentEvent) -> Result<()>;
}

/// Default Unix socket location (`~/.klyntbot/ingest.sock`).
pub const DEFAULT_SOCKET_PATH: &str = "ingest.sock";
/// Default file-buffer location (`~/.klyntbot/ingest-buffer.jsonl`).
pub const DEFAULT_BUFFER_PATH: &str = "ingest-buffer.jsonl";
/// Hard cap for the buffer file before rotation (50 MB).
pub const BUFFER_ROTATE_BYTES: u64 = 50 * 1024 * 1024;
/// Hard-fail ceiling (500 MB).
pub const BUFFER_HARD_CAP_BYTES: u64 = 500 * 1024 * 1024;
/// Buffer file TTL (7 days).
pub const BUFFER_TTL_DAYS: u64 = 7;

/// Unix-domain-socket sink (hot path when klyntbot desktop is running).
#[derive(Debug, Clone)]
pub struct UnixIngestSocket {
    /// Absolute path to the socket file.
    pub path: PathBuf,
}

impl UnixIngestSocket {
    /// Construct with an explicit path.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;
const SEND_TIMEOUT_MS: u64 = 200;

#[async_trait]
impl IngestSocket for UnixIngestSocket {
    async fn send(&self, event: &AgentEvent) -> Result<()> {
        use tokio::io::AsyncWriteExt;
        use tokio::net::UnixStream;
        use tokio::time::{timeout, Duration};

        let body = serde_json::to_vec(event)
            .map_err(|e| KlyntbotError::Storage(format!("serialize: {e}")))?;
        if body.len() > MAX_PAYLOAD_BYTES {
            return Err(KlyntbotError::Storage(format!(
                "payload {} > {} bytes",
                body.len(),
                MAX_PAYLOAD_BYTES
            )));
        }
        let len = u32::try_from(body.len())
            .map_err(|_| KlyntbotError::Storage("payload overflow".into()))?
            .to_le_bytes();

        let dl = Duration::from_millis(SEND_TIMEOUT_MS);
        let mut stream = timeout(dl, UnixStream::connect(&self.path))
            .await
            .map_err(|_| KlyntbotError::Storage("socket connect timeout".into()))?
            .map_err(|e| KlyntbotError::Storage(format!("socket connect: {e}")))?;

        timeout(dl, async {
            stream.write_all(&len).await?;
            stream.write_all(&body).await?;
            stream.shutdown().await?;
            Ok::<_, std::io::Error>(())
        })
        .await
        .map_err(|_| KlyntbotError::Storage("socket write timeout".into()))?
        .map_err(|e| KlyntbotError::Storage(format!("socket write: {e}")))?;

        Ok(())
    }
}

/// File-append sink (cold path when desktop is off).
#[derive(Debug, Clone)]
pub struct FileBufferFallback {
    /// Absolute path to the buffer file.
    pub path: PathBuf,
}

impl FileBufferFallback {
    /// Construct with an explicit path.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[async_trait]
impl IngestSocket for FileBufferFallback {
    async fn send(&self, event: &AgentEvent) -> Result<()> {
        // Hard cap check — primary file only (rotated siblings counted by `prune_older`).
        if let Ok(meta) = tokio::fs::metadata(&self.path).await {
            if meta.len() > BUFFER_HARD_CAP_BYTES {
                return Err(KlyntbotError::Storage(format!(
                    "ingest buffer over hard cap ({} > {} bytes)",
                    meta.len(), BUFFER_HARD_CAP_BYTES
                )));
            }
            if meta.len() > BUFFER_ROTATE_BYTES {
                let rotated = self.path.with_extension(format!(
                    "jsonl.{}",
                    jiff::Timestamp::now().as_millisecond()
                ));
                tokio::fs::rename(&self.path, &rotated).await.map_err(|e| {
                    KlyntbotError::Storage(format!("rotate buffer: {e}"))
                })?;
            }
        }

        let mut line = serde_json::to_vec(event)
            .map_err(|e| KlyntbotError::Storage(format!("serialize: {e}")))?;
        line.push(b'\n');

        let mut f = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await
            .map_err(|e| KlyntbotError::Storage(format!("open buffer: {e}")))?;
        tokio::io::AsyncWriteExt::write_all(&mut f, &line).await
            .map_err(|e| KlyntbotError::Storage(format!("write buffer: {e}")))?;
        tokio::io::AsyncWriteExt::flush(&mut f).await.ok();
        Ok(())
    }
}

impl FileBufferFallback {
    /// Delete rotated sibling files older than `BUFFER_TTL_DAYS`. Safe to call
    /// periodically; errors are logged, never returned. Caller: daemon startup.
    pub async fn prune_older(&self) -> Result<usize> {
        let parent = match self.path.parent() {
            Some(p) => p.to_path_buf(),
            None => return Ok(0),
        };
        let prefix = self.path.file_name()
            .and_then(|n| n.to_str())
            .map(|s| format!("{s}."))
            .unwrap_or_default();
        let ttl = std::time::Duration::from_secs(60 * 60 * 24 * BUFFER_TTL_DAYS);
        let now = std::time::SystemTime::now();
        let mut removed = 0usize;
        let mut rd = tokio::fs::read_dir(&parent).await
            .map_err(|e| KlyntbotError::Storage(format!("readdir: {e}")))?;
        while let Some(entry) = rd.next_entry().await
            .map_err(|e| KlyntbotError::Storage(format!("readdir next: {e}")))?
        {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with(&prefix) { continue; }
            let Ok(meta) = entry.metadata().await else { continue };
            let Ok(modified) = meta.modified() else { continue };
            if now.duration_since(modified).map(|d| d > ttl).unwrap_or(false) {
                let _ = tokio::fs::remove_file(entry.path()).await;
                removed += 1;
            }
        }
        Ok(removed)
    }
}
