//! Klyntbot Core - Foundation types, errors, and utilities
//!
//! This crate provides the foundational types and error handling used across
//! the entire klyntbot workspace.

pub mod entity_card;
pub mod error;
pub mod prompts;
pub mod types;
pub mod utils;

// Re-export commonly used types
pub use entity_card::EntityCard;
pub use error::{
    ChannelError, ConfigError, KlyntbotError, ProviderError, Result, SessionError, ToolError,
};
pub use prompts::{
    Answer, AnswerOption, AnswerType, AnswerValue, FormResponse, InteractionRequest, Question,
};
pub use types::{
    ChannelName, ChatId, MessageRole, SessionKey, CLI_CHANNEL, SYSTEM_CHANNEL,
    TELEGRAM_RESET_SENDER,
};
