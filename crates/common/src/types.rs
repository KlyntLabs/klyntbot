//! Strong domain types for klyntbot.
//!
//! This module provides newtypes and enums to replace primitive string types,
//! improving type safety and preventing common errors.

use serde::{Deserialize, Serialize};
use std::fmt;

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
            _ => MessageRole::User, // Default fallback
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
