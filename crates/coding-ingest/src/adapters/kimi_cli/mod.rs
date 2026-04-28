//! kimi-cli adapter — 13 hook events (tier 1) + Wire streaming (tier 2 stub).
//!
//! Tier-1 dispatch lives in [`dispatch`]; tier-2 streaming surface in [`wire`].
//! `mod.rs` is the thin `IngestAdapter` impl wrapping both.

pub mod dispatch;
mod payload;
pub mod wire;

use super::IngestAdapter;
use crate::event::AgentEvent;
use common::Result;

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
