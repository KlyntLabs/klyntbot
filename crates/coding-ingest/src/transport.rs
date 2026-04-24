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

#[async_trait]
impl IngestSocket for UnixIngestSocket {
    async fn send(&self, _event: &AgentEvent) -> Result<()> {
        Err(KlyntbotError::NotImplemented(
            "UnixIngestSocket::send lands in Phase 2".into(),
        ))
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
    async fn send(&self, _event: &AgentEvent) -> Result<()> {
        Err(KlyntbotError::NotImplemented(
            "FileBufferFallback::send lands in Phase 2".into(),
        ))
    }
}
