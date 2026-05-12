//! Klyntbot Tools — Domain tools and shared infrastructure.
//!
//! Primitive tools (read, write, edit, grep, glob, bash, web_fetch, etc.) now live
//! in `klynt-core`. This crate retains:
//! - Domain tools: learning, memory, project, area, okr, delegation, cron, etc.
//! - Embedding infrastructure: engine (fastembed), store (LanceDB)
//! - Tool registry and parameter utilities
//!
//! Feature-specific tools (tasks, finance) live in their own crates
//! (`feature-tasks`, `feature-finance`) and depend on `tools-core` directly.

// Re-export from tools-core for convenience.
// Consumers can import from tools-core directly.
pub use tools_core::{
    tool_actions, ActionParams, ConfigPersistence, DomainEnum, DynTool, FeatureMigration,
    FeaturePackage, HealthStatus, InteractionBundle, Page, RoutingContext, Searchable, Tool,
};

// Re-export approval_class module from tools-core so domain tools can use crate::approval_class
pub use tools_core::approval_class;

// ── Grouped modules ─────────────────────────────────────────────────────────
pub mod domain;
pub mod embedding;

// ── Module re-exports ───────────────────────────────────────────────────────
pub use domain::{
    agent_task_tool, annotate, area_tool, context_request, cron_tool, docs, learning_tool,
    memory_tool, mirror, okr_tool, project_tool, skill_reference, subagents, temporal,
};
pub use embedding::{embedding_engine, embedding_store};

// ── Shared modules (root-level) ─────────────────────────────────────────────
pub mod conversation_recall;
pub mod progress_handler;
pub mod search_utils;
pub mod semantic_fact_search;
pub mod todo_types;

// ── Tool framework ──────────────────────────────────────────────────────────
pub mod params;
pub mod registry;
pub use params::ParamExtractor;

// ── Re-exports for convenient access by consumers ───────────────────────────

// Agent task
pub use agent_task_tool::{AgentTaskHandler, AgentTaskTool};

// Annotate
pub use annotate::AnnotateTool;

// Area
pub use area_tool::AreaTool;

// Conversation recall
pub use conversation_recall::{
    ConversationRecallHandler, ConversationRecallStatus, PurgeFilter, RecallSearchResult,
};

// Embedding engine
pub use embedding_engine::{EmbeddingEngine, EmbeddingEngineImpl, EmbeddingHandler, EMBEDDING_DIM};
pub use embedding_store::{EmbeddingRecord, EmbeddingStore};

// Learning
pub use learning_tool::{
    LearningHandler, LearningStatus, LearningTool, ThresholdEntry, ToolSummary,
};

// Memory
pub use memory_tool::MemoryTool;

// OKR
pub use okr_tool::OkrTool;

// Progress handler
pub use progress_handler::ProgressHandler;

// Search utilities
pub use search_utils::{rrf_merge, SearchResult};

pub use config::TrustLevel;

// Context request
pub use context_request::{ContextExpansionHandler, ContextRequestTool};

// Docs (content registry)
pub use docs::{ContentRegistryHandler, DocsTool};

// Mirror
pub use mirror::MirrorTool;

// Temporal
pub use temporal::TemporalTool;

// Skill reference
pub use skill_reference::{SkillReferenceIndex, SkillReferenceTool};

// Subagents
pub use subagents::{KillAction, ListAction, ResumeAction, SpawnAction, SubagentsHandler, SubagentsTool};
