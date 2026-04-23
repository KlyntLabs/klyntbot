//! Codex adapter — 5 hook events from OpenAI's Codex CLI.
//!
//! Phase 1 stub. Behavior lands in Phase 7.

use super::IngestAdapter;
use crate::AgentEvent;
use common::{KlyntbotError, Result};

/// Adapter for Codex hook payloads.
#[derive(Debug, Default, Clone, Copy)]
pub struct CodexAdapter;

impl IngestAdapter for CodexAdapter {
    fn source_name(&self) -> &'static str {
        "codex"
    }

    fn parse(&self, _hook_event: &str, _raw: &[u8]) -> Result<Option<AgentEvent>> {
        Err(KlyntbotError::NotImplemented(
            "CodexAdapter::parse lands in Phase 7".into(),
        ))
    }
}
