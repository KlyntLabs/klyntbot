//! klyntbot - A high-performance AI agent framework in Rust

pub mod agent;
pub mod bus;
pub mod channels;
pub mod cli;
pub mod config;
pub mod cron;
pub mod error;
pub mod heartbeat;
pub mod providers;
pub mod session;
pub mod tools;
pub mod utils;

// Re-export commonly used types
pub use agent::{AgentLoop, ContextBuilder, MemoryStore, SkillManager, SubagentManager};
pub use bus::{InboundMessage, MessageBus, OutboundMessage};
pub use channels::{manager::ChannelManager, Channel, DynChannel};
pub use config::Config;
pub use cron::{CronJob, CronService};
pub use error::{KlyntbotError, Result};
pub use heartbeat::HeartbeatService;
pub use providers::{
    create_provider, DynProvider, LlmProvider, LlmResponse, Message, ProviderRegistry,
};
pub use session::{Session, SessionManager};
pub use tools::{DynTool, Tool};

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
