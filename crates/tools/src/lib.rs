//! Klyntbot Tools - Core tool implementations and domain tool interfaces.
//!
//! This crate provides:
//! - Core tool implementations: filesystem (x4), shell, web (x2), message, spawn, cron
//! - Domain tool interfaces: calendar, goal, plan, learning, memory, project
//! - Embedding infrastructure: engine (fastembed), store (LanceDB)
//! - Tool registry and permissions
//!
//! Feature-specific tools (todo, finance) live in their own crates
//! (`feature-todo`, `feature-finance`) and depend on `tools-core` directly.

// Re-export from tools-core for convenience.
// Consumers can import from tools-core directly.
pub use tools_core::{
    tool_actions, ActionParams, ConfigPersistence, DomainEnum, DynTool, FeatureMigration,
    FeaturePackage, HealthStatus, InteractionBundle, Page, RoutingContext, Searchable, Tool,
};

// ── Core tool implementations ────────────────────────────────────────────────
pub mod ask_user;
pub mod browser;
pub mod cron_tool;
pub mod filesystem;
pub mod message;
pub mod shell;
pub mod spawn;
pub mod web;

// ── Domain tool interfaces (handler traits + tool impls) ─────────────────────
pub mod calendar_tool;
pub mod goal_tool;
pub mod learning_tool;
pub mod memory_tool;
pub mod plan_response;
pub mod plan_tool;
pub mod project_tool;
pub mod project_types;

// ── Embedding infrastructure ─────────────────────────────────────────────────
pub mod conversation_embedding;
pub mod embedding_engine;
pub mod embedding_store;

// ── Shared types still consumed by tools-internal modules ────────────────────
pub mod search_utils;
pub mod todo_types;

// ── Tool framework ───────────────────────────────────────────────────────────
pub mod params;
pub mod permissions;
pub mod registry;
pub use params::ParamExtractor;

// ── Re-exports for convenient access by consumers ────────────────────────────

// Calendar
pub use calendar_tool::{CalendarHandler, CalendarTool};

// Conversation embedding
pub use conversation_embedding::{
    ConversationEmbeddingHandler, ConversationEmbeddingRecord, ConversationEmbeddingStatus,
    ConversationEmbeddingStore, PurgeFilter,
};

// Embedding engine
pub use embedding_engine::{EmbeddingEngine, EmbeddingEngineImpl, EmbeddingHandler, EMBEDDING_DIM};
pub use embedding_store::{EmbeddingRecord, EmbeddingStore};

// Goal
pub use goal_tool::{GoalHandler, GoalTool};

// Learning
pub use learning_tool::{
    LearningHandler, LearningStatus, LearningTool, ThresholdEntry, ToolSummary,
};

// Plan
pub use plan_tool::{PlanCompletionHandler, PlanHandler, PlanTool};

// Memory
pub use memory_tool::MemoryTool;

// Permissions
pub use permissions::{PermissionLevel, ToolPermissions};

// Search utilities
pub use search_utils::{rrf_merge, SearchResult};

// Browser
pub use browser::BrowserTool;
pub use config::TrustLevel;
