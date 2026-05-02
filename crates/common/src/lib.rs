//! Klyntbot Core — foundation types, errors, and utilities.
//!
//! This crate provides the foundational types and error handling used across
//! the entire klyntbot workspace.

// ── Modules ─────────────────────────────────────────────────────────────

pub mod autotuner;
pub mod date;
pub mod entity_card;
pub mod error;
pub mod helpers;
pub mod http;
pub mod memory;
pub mod notify;
pub mod ports;
pub mod pricing;
pub mod prompts;
pub mod time;
pub mod tool_channel;
pub mod types;

// ── Re-exports: domain types ────────────────────────────────────────────

pub use autotuner::TrialParams;
pub use entity_card::EntityCard;
pub use error::{
    ChannelError, ConfigError, KlyntbotError, ProviderError, Result, SessionError, ToolError,
};
pub use prompts::{
    Answer, AnswerOption, AnswerType, AnswerValue, FormResponse, InteractionRequest, Question,
};
pub use tool_channel::{Channel, ChannelMask};
pub use types::{
    AppMode, ChannelName, ChatId, MessageRole, SessionKey, CLI_CHANNEL, CODING_CHANNEL,
    MCP_CHANNEL, MIRROR_ALERT_COST_THRESHOLD_CROSSED, SYSTEM_CHANNEL, TELEGRAM_RESET_SENDER,
};

// ── Re-exports: commonly used utilities ─────────────────────────────────

pub use rust_decimal::Decimal;

pub use date::parse_datetime_jiff;
pub use helpers::{truncate_at_boundary, truncate_chars};
pub use http::{build_http_client, build_http_client_with_builder, shared_http_client};
pub use ports::NotificationSender;
