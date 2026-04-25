//! `MemorySink` — the abstraction native in-process consumers (klynt-cli)
//! and the ingest socket share. Lets klynt-cli emit events directly to the
//! Distiller when desktop is off, and to the socket when desktop is alive.
//!
//! See coding-memory design §5 "Native source: klynt-cli".

use crate::distiller::Distiller;
use async_trait::async_trait;
use coding_ingest::AgentEvent;
use common::{KlyntbotError, Result};
use std::path::PathBuf;
use std::sync::Arc;

/// Abstraction over "accept an `AgentEvent` from a native source".
#[async_trait]
pub trait MemorySink: Send + Sync {
    /// Accept one event. Implementations buffer / forward as appropriate.
    async fn accept_event(&self, event: AgentEvent) -> Result<()>;
    /// Flush any pending events — called at session end or on shutdown.
    async fn flush(&self) -> Result<()>;
}

/// In-process sink — forwards events to an injected `Distiller`.
#[derive(Debug, Clone)]
pub struct InProcessSink {
    distiller: Option<Arc<Distiller>>,
}

impl InProcessSink {
    /// Construct a sink with no distiller wired (events are dropped).
    #[must_use]
    pub fn new() -> Self {
        Self { distiller: None }
    }

    /// Wire a distiller handle — called during `AppCore::init`.
    pub fn set_distiller(&mut self, distiller: Arc<Distiller>) {
        self.distiller = Some(distiller);
    }
}

impl Default for InProcessSink {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MemorySink for InProcessSink {
    async fn accept_event(&self, event: AgentEvent) -> Result<()> {
        if let Some(d) = &self.distiller {
            d.accept_event(event).await?;
        }
        Ok(())
    }
    async fn flush(&self) -> Result<()> {
        if let Some(d) = &self.distiller {
            d.sweep_idle().await?;
        }
        Ok(())
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
    KlyntbotError::NotImplemented(format!("sink not implemented until phase {p}"))
}
