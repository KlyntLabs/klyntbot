//! Session memory context source — injects the per-session conversation scratchpad.
//!
//! Reads the `session_memory` table entry for the current session key (`ctx.chat_id`)
//! and injects the structured markdown summary into the system prompt. This gives the
//! LLM awareness of the ongoing conversation state (current task, decisions, progress)
//! without re-reading the full message history every turn.

use async_trait::async_trait;
use context_engine::source::{ContextSource, SourceContext};

/// Injects the auto-tracked conversation scratchpad into the system prompt.
///
/// Priority 88 — between identity context (100) and session entity context (85).
pub struct SessionMemoryContextSource {
    repo: storage::SessionMemoryRepo,
}

impl SessionMemoryContextSource {
    pub fn new(repo: storage::SessionMemoryRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl ContextSource for SessionMemoryContextSource {
    fn name(&self) -> &str {
        "session_memory"
    }

    fn priority(&self) -> u8 {
        88
    }

    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        let row = self.repo.get(&ctx.chat_id).await.ok().flatten()?;
        if row.content.is_empty() {
            return None;
        }
        Some(format!(
            "# Conversation State (auto-tracked)\n\n{}\n\n_Updated at turn {}_",
            row.content, row.turn_count
        ))
    }
}
