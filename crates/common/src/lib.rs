//! Klyntbot Core - Foundation types, errors, and utilities
//!
//! This crate provides the foundational types and error handling used across
//! the entire klyntbot workspace.

pub mod error;
pub mod interaction;
pub mod prompts;
pub mod types;
pub mod utils;

// Re-export commonly used types
pub use error::{
    ChannelError, ConfigError, CronError, KlyntbotError, ProviderError, Result, SessionError,
    ToolError,
};
pub use interaction::InteractionRenderer;
pub use prompts::{
    Answer, AnswerOption, AnswerType, AnswerValue, FormResponse, InteractionRequest, Question,
};
pub use types::{ChannelName, ChatId, MessageRole, SessionKey};
