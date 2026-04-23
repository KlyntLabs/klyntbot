//! opencode adapter — SQLite WAL polling (500ms) over opencode's local DB.
//!
//! Phase 1 stub. Behavior lands in Phase 7.

use super::IngestAdapter;
use crate::AgentEvent;
use common::{KlyntbotError, Result};

/// Adapter for opencode SQLite polling.
#[derive(Debug, Default, Clone, Copy)]
pub struct OpencodeAdapter;

impl IngestAdapter for OpencodeAdapter {
    fn source_name(&self) -> &'static str {
        "opencode"
    }

    fn parse(&self, _hook_event: &str, _raw: &[u8]) -> Result<Option<AgentEvent>> {
        Err(KlyntbotError::NotImplemented(
            "OpencodeAdapter::parse lands in Phase 7".into(),
        ))
    }
}
