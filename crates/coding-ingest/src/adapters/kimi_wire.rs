//! kimi-cli adapter — 13 hook events (tier 1) + Wire streaming (tier 2).
//!
//! Phase 1 stub. Behavior lands in Phase 7.

use super::IngestAdapter;
use crate::AgentEvent;
use common::{KlyntbotError, Result};

/// Adapter for kimi-cli hook payloads. Wire-tier client surface lands later.
#[derive(Debug, Default, Clone, Copy)]
pub struct KimiAdapter;

impl IngestAdapter for KimiAdapter {
    fn source_name(&self) -> &'static str {
        "kimi-cli"
    }

    fn parse(&self, _hook_event: &str, _raw: &[u8]) -> Result<Option<AgentEvent>> {
        Err(KlyntbotError::NotImplemented(
            "KimiAdapter::parse lands in Phase 7".into(),
        ))
    }
}
