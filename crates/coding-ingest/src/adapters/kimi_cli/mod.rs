//! kimi-cli adapter — delegates to the Claude Code parser since kimi-cli's
//! hook system is a 1:1 clone (same 9 events, same JSON payload shapes,
//! same `~/.kimi/config.toml` `[[hooks]]` configuration model).
//!
//! Tier-2 Wire streaming surface lives in [`wire`] and is independent.

pub mod wire;
// Legacy modules kept on disk to avoid churn in tests/imports; not re-exported.
#[allow(dead_code)]
pub mod dispatch;
#[allow(dead_code)]
mod payload;

use super::claude_code::ClaudeCodeAdapter;
use super::IngestAdapter;
use crate::event::{AgentEvent, AgentSource};
use common::Result;

/// Spawn the Tier-2 Wire streaming loop. Returns a JoinHandle the caller
/// (typically the daemon) can drop to cancel.
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
        match ClaudeCodeAdapter.parse(hook_event, raw)? {
            Some(AgentEvent::V1(mut v1)) => {
                v1.source = AgentSource::KimiCli;
                Ok(Some(AgentEvent::V1(v1)))
            }
            None => Ok(None),
        }
    }
}
