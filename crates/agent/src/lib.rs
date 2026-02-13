//! Klyntbot Agent - Core agent orchestration
//!
//! This crate provides the AgentLoop and related agent functionality.

pub mod agent_loop;
pub mod context;
pub mod cron_handler_adapter;
pub mod events;
pub mod memory;
pub mod notifications;
pub mod skills;
pub mod subagent;

pub use agent_loop::AgentLoop;
pub use context::ContextBuilder;
pub use cron_handler_adapter::CronHandlerAdapter;
pub use events::AgentEvent;
pub use memory::MemoryStore;
pub use notifications::NotificationDispatcher;
pub use skills::SkillManager;
pub use subagent::SubagentManager;

// Re-export prompt types from common for convenience
pub use common::{PromptRequest, PromptType, UserResponse};
