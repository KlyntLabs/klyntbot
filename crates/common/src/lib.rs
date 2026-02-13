//! Klyntbot Core - Foundation types, errors, and utilities
//!
//! This crate provides the foundational types and error handling used across
//! the entire klyntbot workspace.

pub mod error;
pub mod prompts;
pub mod types;
pub mod utils;

// Re-export commonly used types
pub use error::{
    ChannelError, ConfigError, CronError, KlyntbotError, ProviderError, Result, SessionError,
    ToolError,
};
pub use prompts::{PromptOption, PromptOptionWithInput, PromptRequest, PromptType, UserResponse};
pub use types::{ChannelName, ChatId, MessageRole, SessionKey};
