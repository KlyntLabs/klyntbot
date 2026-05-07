//! Cross-CLI agent-tracing module.
//!
//! v1 implements only the Kimi provider; the trait and DTOs are
//! deliberately CLI-agnostic so Codex/Claude Code/opencode adapters
//! drop in without UI changes.

pub mod provider;
pub mod providers;
pub mod registry;
pub mod types;

pub use provider::TracingProvider;
pub use registry::TracingRegistry;
pub use types::{
    ContextMessage, ErrorByTool, HeaderChip, HeaderStats, KimiTodo, ProjectTotals, ProviderInfo,
    Scope, SemanticCategory, SessionDetail, SessionState, SessionSummary, SessionTab, StatsBundle,
    SubagentSummary, SubagentTypeCount, TokenSeriesPoint, ToolUsage, TraceEvent,
};
