//! Pluggable context source trait for system prompt assembly.
//!
//! The `ContextSource` trait defines a provider of context sections
//! that are assembled into the system prompt. Implementations live
//! in downstream crates (e.g., `agent`) — this is the same dependency
//! inversion pattern used by `SpawnHandler` and `CronHandler`.

use async_trait::async_trait;

/// Per-request metadata passed to context sources.
#[derive(Debug, Clone)]
pub struct SourceContext {
    /// Channel name (e.g., "telegram", "discord", "cli").
    pub channel: String,
    /// Chat/conversation ID.
    pub chat_id: String,
    /// Current user message (for relevance-filtered sources).
    pub message: Option<String>,
    /// Project ID for project-scoped context sources.
    pub project_id: Option<String>,
    /// Session mode discriminator (assistant vs coding).
    pub session_mode: common::SessionMode,
}

/// A pluggable provider of context sections for the system prompt.
///
/// Sources are sorted by priority (higher = included first) and each
/// produces an optional string section. Sections are joined with
/// `\n\n---\n\n` separators to form the complete system prompt.
///
/// Caching is the responsibility of each source implementation.
#[async_trait]
pub trait ContextSource: Send + Sync {
    /// Human-readable name for logging/debugging.
    fn name(&self) -> &str;

    /// Priority for ordering: higher values appear earlier in the prompt.
    fn priority(&self) -> u8;

    /// Produce a context section, or `None` to skip.
    async fn provide(&self, ctx: &SourceContext) -> Option<String>;

    /// Estimated token count for this source's output.
    /// Used for budget planning before actually loading.
    /// Default: 500 tokens (conservative estimate).
    fn estimated_tokens(&self) -> usize {
        500
    }

    /// Whether this source's content should be protected from context compaction.
    /// Protected content is never pruned when the context window fills up.
    /// Default: false (content can be pruned if needed).
    fn protected(&self) -> bool {
        false
    }
}
