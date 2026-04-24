//! Desktop-embedded ingestion daemon — owns the `ingest.sock` lifecycle
//! and drains events into `ingest_event_log`.
//!
//! Phase 1 stub. Behavior lands in Phase 2 (lifecycle + Claude Code E2E).

use crate::AgentEvent;
use common::{KlyntbotError, Result};
use std::path::PathBuf;

/// Configuration for the ingestion daemon.
#[derive(Debug, Clone)]
pub struct IngestDaemonConfig {
    /// Where the Unix socket is bound.
    pub socket_path: PathBuf,
    /// Where the cold-path file buffer lives.
    pub buffer_path: PathBuf,
}

/// Daemon handle — obtained after `spawn`; used to shutdown cleanly.
#[derive(Debug)]
pub struct IngestDaemonHandle {
    /// Placeholder — Phase 2 replaces with a shutdown sender.
    _private: (),
}

/// Spawn the ingestion daemon. Owned by the klyntbot desktop binary.
///
/// Phase 1 stub — returns an error so the desktop-layer wiring can reference
/// the symbol without Phase 1 regressing desktop startup. Desktop does not
/// yet call this in Phase 1.
pub async fn spawn(_cfg: IngestDaemonConfig) -> Result<IngestDaemonHandle> {
    Err(KlyntbotError::NotImplemented(
        "ingest daemon spawn lands in Phase 2".into(),
    ))
}

/// Record drainage API — exposed for `ingest_event_log` replay. Phase 2.
pub async fn drain_buffer(_path: &PathBuf) -> Result<Vec<AgentEvent>> {
    Err(KlyntbotError::NotImplemented(
        "buffer drain lands in Phase 2".into(),
    ))
}
