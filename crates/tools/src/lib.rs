//! Klyntbot Tools - Tool system for agent capabilities
//!
//! This crate provides the Tool trait and various tool implementations.

// The FinanceTool parameters() JSON schema is large; increase the macro recursion limit.
#![recursion_limit = "256"]

// Re-export from tools-core for backward compatibility.
// Consumers should gradually migrate to importing from tools-core directly.
// Tool, DynTool, RoutingContext, and InteractionBundle are re-exported so that any
// type implementing tools_core::Tool (e.g. feature_todo::TodoTool, feature_finance::FinanceTool)
// automatically satisfies tools::Tool and can be registered in ToolRegistry.
pub use tools_core::{
    tool_actions,
    ActionParams,
    ConfigPersistence,
    DomainEnum,
    DynTool,
    FeatureMigration,
    FeaturePackage,
    HealthStatus,
    InteractionBundle,
    Page,
    RoutingContext,
    Searchable,
    // Tool system: re-exported so tools::Tool == tools_core::Tool
    Tool,
};

pub mod ask_user;
pub mod calendar_tool;
pub mod conversation_embedding;
pub mod cron_tool;
pub mod embedding_engine;
pub mod embedding_store;
pub mod enrichment;
pub mod filesystem;
pub mod finance_handler;
pub mod finance_tool;
pub mod finance_types;
pub mod goal_tool;
pub mod learning_feedback;
pub mod learning_tool;
pub mod memory_tool;
pub mod message;
pub mod params;
pub mod plan_response;
pub mod plan_tool;
pub mod price_service;
pub use params::ParamExtractor;
pub mod permissions;
pub mod project_tool;
pub mod project_types;
pub mod registry;
pub mod rrule_utils;
pub mod search_utils;
pub mod shell;
pub mod spawn;
pub mod todo;
pub mod todo_types;
pub mod web;

// Re-export calendar types for use by agent crate
pub use calendar_tool::{CalendarHandler, CalendarTool};

// Re-export finance handler types for use by agent crate
pub use finance_handler::{BudgetAlert, FinanceHandler, PriceUpdateSummary, ProactivityLevel};

// Re-export finance domain types
pub use finance_types::{
    AccountType, AssetType, BudgetMethod, BudgetPeriod, FinanceAccount, FinanceBudget, FinanceGoal,
    FinanceInvestment, FinanceInvestmentDomainFilter, FinanceInvestmentTx, FinanceLiability,
    FinancePortfolio, FinanceTransaction, FinanceTransactionFilter, GoalStatus, GoalType,
    InvestmentTxType, JarType, LiabilityType, TransactionType,
};

// Re-export FinanceTool
pub use finance_tool::FinanceTool;

// Re-export price service types for use by agent crate
pub use price_service::{CachedPrice, PriceResult, PriceService};

// Re-export conversation embedding types for use by agent crate
pub use conversation_embedding::{
    ConversationEmbeddingHandler, ConversationEmbeddingRecord, ConversationEmbeddingStatus,
    ConversationEmbeddingStore, PurgeFilter,
};

// Re-export embedding types for use by agent crate
pub use embedding_engine::{EmbeddingEngine, EmbeddingEngineImpl, EmbeddingHandler, EMBEDDING_DIM};
pub use embedding_store::{EmbeddingRecord, EmbeddingStore};

// Re-export enrichment types for use by agent crate
pub use enrichment::{EnrichmentHandler, EnrichmentResult, EnrichmentSuggestion};

// Re-export learning feedback types for use by agent crate
pub use learning_feedback::{EnrichmentFeedbackEntry, EnrichmentFeedbackHandler};

// Re-export goal types for use by agent crate
pub use goal_tool::{GoalHandler, GoalTool};

// Re-export learning tool types for use by agent crate
pub use learning_tool::{
    LearningHandler, LearningStatus, LearningTool, ThresholdEntry, ToolSummary,
};

// Re-export plan types for use by agent crate
pub use plan_tool::{PlanCompletionHandler, PlanHandler, PlanTool};

// Re-export memory tool
pub use memory_tool::MemoryTool;

// Re-export permissions types
pub use permissions::{PermissionLevel, ToolPermissions};

// Re-export rrule types
pub use rrule_utils::{Frequency, RRule};

// Re-export search utilities
pub use search_utils::{rrf_merge, SearchResult};
