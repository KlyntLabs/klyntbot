//! kimi-cli adapter — 13 hook events (tier 1) + Wire streaming (tier 2).
//!
//! Tier-1 dispatch lives in [`dispatch`]; tier-2 streaming surface in [`wire`].
//! `mod.rs` is the thin `IngestAdapter` impl wrapping both.

pub mod dispatch;
mod payload;
pub mod wire;

use super::IngestAdapter;
use crate::event::AgentEvent;
use common::Result;

/// Spawn the Tier-2 Wire streaming loop. Returns a JoinHandle the caller
/// (typically the daemon) can drop to cancel. If the socket is unavailable
/// the future returns immediately with an Io error — tier-1 hooks remain
/// the fallback.
pub fn spawn_wire(
    socket_path: std::path::PathBuf,
    tx: tokio::sync::mpsc::UnboundedSender<crate::event::AgentEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = wire::run(socket_path, tx).await {
            tracing::warn!(error = %e, "kimi wire loop exited");
        }
    })
}

/// Adapter for kimi-cli hook payloads.
#[derive(Debug, Default, Clone, Copy)]
pub struct KimiAdapter;

impl IngestAdapter for KimiAdapter {
    fn source_name(&self) -> &'static str {
        "kimi-cli"
    }

    fn parse(&self, hook_event: &str, raw: &[u8]) -> Result<Option<AgentEvent>> {
        Ok(dispatch::dispatch(hook_event, raw)?.map(AgentEvent::V1))
    }
}
