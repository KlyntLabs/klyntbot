//! `MemorySink` — the abstraction native in-process consumers (klynt-cli)
//! and the ingest socket share. Lets klynt-cli emit events directly to the
//! Distiller when desktop is off, and to the socket when desktop is alive.
//!
//! See coding-memory design §5 "Native source: klynt-cli".

use crate::error::NotImplementedInPhase;
use async_trait::async_trait;
use coding_ingest::AgentEvent;
use common::{KlyntbotError, Result};
use std::path::PathBuf;

/// Abstraction over "accept an `AgentEvent` from a native source".
#[async_trait]
pub trait MemorySink: Send + Sync {
    /// Accept one event. Implementations buffer / forward as appropriate.
    async fn accept_event(&self, event: AgentEvent) -> Result<()>;
    /// Flush any pending events — called at session end or on shutdown.
    async fn flush(&self) -> Result<()>;
}

/// In-process sink — when desktop is off, klynt-cli calls the Distiller directly.
#[derive(Debug, Default, Clone)]
pub struct InProcessSink {
    /// Phase-2+ wiring will carry a `Distiller` handle here.
    _phase_stub: (),
}

impl InProcessSink {
    /// Construct an in-process sink. Phase 1 stub.
    #[must_use]
    pub fn new() -> Self {
        Self { _phase_stub: () }
    }
}

#[async_trait]
impl MemorySink for InProcessSink {
    async fn accept_event(&self, _event: AgentEvent) -> Result<()> {
        Err(phase(2))
    }
    async fn flush(&self) -> Result<()> {
        Err(phase(2))
    }
}

/// Unix-socket sink — when desktop is alive, klynt-cli writes to `ingest.sock`.
#[derive(Debug, Clone)]
pub struct IngestSocketSink {
    /// Socket path.
    pub socket_path: PathBuf,
}

impl IngestSocketSink {
    /// Construct with an explicit socket path.
    #[must_use]
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }
}

#[async_trait]
impl MemorySink for IngestSocketSink {
    async fn accept_event(&self, _event: AgentEvent) -> Result<()> {
        Err(phase(2))
    }
    async fn flush(&self) -> Result<()> {
        Err(phase(2))
    }
}

fn phase(p: u8) -> KlyntbotError {
    KlyntbotError::NotImplemented(format!("{:?}", NotImplementedInPhase::new(p)))
}
