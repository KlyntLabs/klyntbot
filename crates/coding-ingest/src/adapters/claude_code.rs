//! Claude Code adapter — 7 hook events filtered from Claude's 27.
//!
//! Phase 1 stub. Behavior lands in Phase 2.

use super::IngestAdapter;
use crate::AgentEvent;
use common::{KlyntbotError, Result};

/// Adapter for Claude Code hook payloads.
#[derive(Debug, Default, Clone, Copy)]
pub struct ClaudeCodeAdapter;

impl IngestAdapter for ClaudeCodeAdapter {
    fn source_name(&self) -> &'static str {
        "claude-code"
    }

    fn parse(&self, _hook_event: &str, _raw: &[u8]) -> Result<Option<AgentEvent>> {
        Err(KlyntbotError::NotImplemented(
            "ClaudeCodeAdapter::parse lands in Phase 2".into(),
        ))
    }
}
