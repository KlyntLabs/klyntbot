//! CLI-specific adapters normalize per-CLI hook payloads to `AgentEvent`.
//!
//! Phase 1 ships trait + empty adapters. Each phase-2+ task fleshes out
//! one adapter; the trait signature is stable from Phase 1.

use crate::AgentEvent;
use common::Result;

/// Adapter that converts one CLI's per-hook stdin payload into an `AgentEvent`.
///
/// Implementations are stateless; one instance can handle many hook invocations.
pub trait IngestAdapter: Send + Sync {
    /// Stable name used in `AgentSource` and settings UI.
    fn source_name(&self) -> &'static str;

    /// Parse a single stdin payload + originating hook event name.
    fn parse(&self, hook_event: &str, raw: &[u8]) -> Result<Option<AgentEvent>>;
}

/// Claude Code adapter.
pub mod claude_code;
/// Codex adapter.
pub mod codex;
/// Git post-commit adapter.
pub mod git_post_commit;
/// kimi-cli adapter (tier-1 hook + tier-2 Wire path).
pub mod kimi_cli;
/// opencode adapter (SQLite WAL polling).
pub mod opencode;
