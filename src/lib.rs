//! Klyntbot - AI agent platform for multi-channel messaging
//!
//! This is the root library crate that re-exports all workspace crates.

// Re-export all workspace crates
pub use agent;
pub use bus;
pub use channels;
pub use cognitive;
pub use common;
pub use config;
pub use context_engine;
pub use database_tool;
pub use entity_store;
pub use feature_coaching;
pub use mcp;
pub use providers;
pub use scheduling;
pub use session;
pub use storage;
pub use tools;

// Re-export commonly used types for convenience
pub use agent::{
    AgentEvent, AgentLoop, ConfidenceSource, FinanceHandlerImpl, ProgressHandlerImpl,
    StreamingHandle, SubagentManager,
};
pub use bus::{InboundMessage, MessageBus, OutboundMessage};
pub use channels::{Channel, ChannelManager, DynChannel};
pub use common::error;
pub use common::{
    Answer, AnswerOption, AnswerType, AnswerValue, ChannelName, ChatId, FormResponse,
    InteractionRequest, MessageRole, Question, Result, SessionKey,
};
pub use config::Config;
pub use providers::{
    create_provider, create_provider_with_failover, DynProvider, LlmProvider, LlmResponse, Message,
    ProviderRegistry,
};
pub use scheduling::{CronJob, CronService};
pub use session::{Session, SessionManager};
pub use storage::{Repos, StoragePool};
pub use tools::{DynTool, Tool};

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
