//! Strong domain types for klyntbot.
//!
//! This module provides newtypes and enums to replace primitive string types,
//! improving type safety and preventing common errors.

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::KlyntbotError;

// ── Well-known channel / sender constants ─────────────────────────────────
pub const SYSTEM_CHANNEL: &str = "system";
pub const CLI_CHANNEL: &str = "cli";
pub const MCP_CHANNEL: &str = "mcp";
pub const CODING_CHANNEL: &str = "coding";
pub const TELEGRAM_RESET_SENDER: &str = "telegram_reset";

/// Mirror alert kind emitted when a coding session's per-thread cost ceiling is crossed.
pub const MIRROR_ALERT_COST_THRESHOLD_CROSSED: &str = "costThresholdCrossed";

/// Channel name (e.g., "telegram", "discord")
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelName(String);

impl ChannelName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ChannelName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for ChannelName {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ChannelName {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Chat identifier within a channel
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChatId(String);

impl ChatId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ChatId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for ChatId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ChatId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Session key (channel:chat_id)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionKey(String);

impl SessionKey {
    pub fn new(channel: &ChannelName, chat_id: &ChatId) -> Self {
        Self(format!("{}:{}", channel.as_str(), chat_id.as_str()))
    }

    pub fn from_parts(channel: &str, chat_id: &str) -> Self {
        Self(format!("{}:{}", channel, chat_id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Split the session key into channel and chat_id parts
    pub fn split(&self) -> Option<(ChannelName, ChatId)> {
        let parts: Vec<&str> = self.0.splitn(2, ':').collect();
        if parts.len() == 2 {
            Some((ChannelName::from(parts[0]), ChatId::from(parts[1])))
        } else {
            None
        }
    }
}

impl fmt::Display for SessionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for SessionKey {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for SessionKey {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Runtime mode for the application entry point.
/// Controls which subsystems are initialized during `AppCore::init()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppMode {
    /// Full desktop app: channels, productivity, coaching, everything.
    #[default]
    Desktop,
    /// Headless MCP server: storage, agent, cron only.
    Server,
}

/// Message role in conversations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

impl fmt::Display for MessageRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MessageRole::System => write!(f, "system"),
            MessageRole::User => write!(f, "user"),
            MessageRole::Assistant => write!(f, "assistant"),
            MessageRole::Tool => write!(f, "tool"),
        }
    }
}

impl From<&str> for MessageRole {
    fn from(s: &str) -> Self {
        match s {
            "system" => MessageRole::System,
            "user" => MessageRole::User,
            "assistant" => MessageRole::Assistant,
            "tool" => MessageRole::Tool,
            _ => {
                tracing::warn!("Unknown message role '{}', defaulting to User", s);
                MessageRole::User
            }
        }
    }
}

impl MessageRole {
    /// Strict parsing that returns an error for unknown role strings.
    ///
    /// Unlike `From<&str>` which silently defaults to `User`, this method
    /// rejects unknown values — useful at system boundaries.
    pub fn parse_strict(s: &str) -> crate::Result<Self> {
        match s {
            "system" => Ok(MessageRole::System),
            "user" => Ok(MessageRole::User),
            "assistant" => Ok(MessageRole::Assistant),
            "tool" => Ok(MessageRole::Tool),
            _ => Err(KlyntbotError::Tool(crate::ToolError::InvalidParams(
                format!("Unknown message role: '{}'", s),
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_role_from_known_strings() {
        assert_eq!(MessageRole::from("system"), MessageRole::System);
        assert_eq!(MessageRole::from("user"), MessageRole::User);
        assert_eq!(MessageRole::from("assistant"), MessageRole::Assistant);
        assert_eq!(MessageRole::from("tool"), MessageRole::Tool);
    }

    #[test]
    fn test_message_role_from_unknown_defaults_to_user() {
        assert_eq!(MessageRole::from("unknown"), MessageRole::User);
        assert_eq!(MessageRole::from(""), MessageRole::User);
        assert_eq!(MessageRole::from("SYSTEM"), MessageRole::User);
    }

    #[test]
    fn test_message_role_parse_strict_known_strings() {
        assert_eq!(
            MessageRole::parse_strict("system").unwrap(),
            MessageRole::System
        );
        assert_eq!(
            MessageRole::parse_strict("user").unwrap(),
            MessageRole::User
        );
        assert_eq!(
            MessageRole::parse_strict("assistant").unwrap(),
            MessageRole::Assistant
        );
        assert_eq!(
            MessageRole::parse_strict("tool").unwrap(),
            MessageRole::Tool
        );
    }

    #[test]
    fn test_message_role_parse_strict_unknown_returns_error() {
        let err = MessageRole::parse_strict("unknown").unwrap_err();
        assert!(err.to_string().contains("Unknown message role: 'unknown'"));

        let err = MessageRole::parse_strict("").unwrap_err();
        assert!(err.to_string().contains("Unknown message role: ''"));

        let err = MessageRole::parse_strict("SYSTEM").unwrap_err();
        assert!(err.to_string().contains("Unknown message role: 'SYSTEM'"));
    }

    #[test]
    fn test_message_role_display() {
        assert_eq!(MessageRole::System.to_string(), "system");
        assert_eq!(MessageRole::User.to_string(), "user");
        assert_eq!(MessageRole::Assistant.to_string(), "assistant");
        assert_eq!(MessageRole::Tool.to_string(), "tool");
    }

    #[test]
    fn test_session_key() {
        let channel = ChannelName::new("telegram");
        let chat_id = ChatId::new("123456");
        let session_key = SessionKey::new(&channel, &chat_id);

        assert_eq!(session_key.as_str(), "telegram:123456");
        assert_eq!(session_key.to_string(), "telegram:123456");

        // Test splitting
        let (parsed_channel, parsed_chat_id) = session_key.split().unwrap();
        assert_eq!(parsed_channel, channel);
        assert_eq!(parsed_chat_id, chat_id);
    }

    #[test]
    fn coding_channel_constant_value() {
        assert_eq!(CODING_CHANNEL, "coding");
    }

    #[test]
    fn coding_channel_round_trips_through_channel_name() {
        let channel = ChannelName::new(CODING_CHANNEL);
        assert_eq!(channel.as_str(), "coding");
    }
}
