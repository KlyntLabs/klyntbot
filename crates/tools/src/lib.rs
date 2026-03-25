//! Klyntbot Tools - Core tool implementations and domain tool interfaces.
//!
//! This crate provides:
//! - System tools: filesystem (x4), web (x2), browser, message, ask_user
//! - Domain tools: learning, memory, project, area, okr, delegation, cron, etc.
//! - Embedding infrastructure: engine (fastembed), store (LanceDB)
//! - Tool registry and permissions
//!
//! Feature-specific tools (tasks, finance) live in their own crates
//! (`feature-tasks`, `feature-finance`) and depend on `tools-core` directly.

// Re-export from tools-core for convenience.
// Consumers can import from tools-core directly.
pub use tools_core::{
    tool_actions, ActionParams, ConfigPersistence, DomainEnum, DynTool, FeatureMigration,
    FeaturePackage, HealthStatus, InteractionBundle, Page, RoutingContext, Searchable, Tool,
};

// ── Grouped modules ─────────────────────────────────────────────────────────
pub mod domain;
pub mod embedding;
pub mod system;

// ── Module re-exports (backward-compatible paths) ───────────────────────────
pub use domain::{
    agent_task_tool, annotate, area_tool, context_request, cron_tool, delegation, docs,
    learning_tool, memory_tool, mirror, okr_tool, project_tool, skill_reference, spawn,
};
pub use embedding::{embedding_engine, embedding_store};
pub use system::{ask_user, browser, filesystem, glob_tool, grep, message, web};

// ── Shared modules (root-level) ─────────────────────────────────────────────
pub mod conversation_recall;
pub mod progress_handler;
pub mod search_utils;
pub mod todo_types;

// ── Tool framework ──────────────────────────────────────────────────────────
pub mod params;
pub mod permissions;
pub mod registry;
pub use params::ParamExtractor;

// ── Re-exports for convenient access by consumers ───────────────────────────

// Agent task
pub use agent_task_tool::{AgentTaskHandler, AgentTaskTool};

// Annotate
pub use annotate::AnnotateTool;

// Area
pub use area_tool::AreaTool;

// Delegation
pub use delegation::{DelegationHandler, DelegationTool};

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

// Permissions
pub use permissions::{PermissionLevel, ToolPermissions};

// Progress handler
pub use progress_handler::ProgressHandler;

// Search utilities
pub use search_utils::{rrf_merge, SearchResult};

// Browser
pub use browser::BrowserTool;
pub use config::TrustLevel;

// Context request
pub use context_request::{ContextExpansionHandler, ContextRequestTool};

// Docs (content registry)
pub use docs::{ContentRegistryHandler, DocsTool};

// Mirror
pub use mirror::MirrorTool;

// Skill reference
pub use skill_reference::{SkillReferenceIndex, SkillReferenceTool};
