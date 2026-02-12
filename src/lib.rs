//! Klyntbot - AI agent platform for multi-channel messaging
//!
//! This is the root library crate that re-exports all workspace crates.

// Re-export all workspace crates as modules
pub use klyntbot_agent as agent;
pub use klyntbot_bus as bus;
pub use klyntbot_channels as channels;
pub use klyntbot_cli as cli;
pub use klyntbot_config as config;
pub use klyntbot_core as core;
pub use klyntbot_cron as cron;
pub use klyntbot_heartbeat as heartbeat;
pub use klyntbot_providers as providers;
pub use klyntbot_session as session;
pub use klyntbot_tools as tools;

// Re-export commonly used types for convenience
pub use klyntbot_core::error;
pub use klyntbot_core::{ChannelName, ChatId, MessageRole, Result, SessionKey};
pub use klyntbot_agent::{AgentLoop, ContextBuilder, MemoryStore, SkillManager, SubagentManager};
pub use klyntbot_bus::{InboundMessage, MessageBus, OutboundMessage};
pub use klyntbot_channels::{Channel, ChannelManager, DynChannel};
pub use klyntbot_config::Config;
pub use klyntbot_cron::{CronJob, CronService};
pub use klyntbot_heartbeat::HeartbeatService;
pub use klyntbot_providers::{create_provider, DynProvider, LlmProvider, LlmResponse, Message, ProviderRegistry};
pub use klyntbot_session::{Session, SessionManager};
pub use klyntbot_tools::{DynTool, Tool};

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
