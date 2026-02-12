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
    fn test_channel_name() {
        let channel = ChannelName::new("telegram");
        assert_eq!(channel.as_str(), "telegram");
        assert_eq!(channel.to_string(), "telegram");
    }

    #[test]
    fn test_chat_id() {
        let chat_id = ChatId::new("123456");
        assert_eq!(chat_id.as_str(), "123456");
        assert_eq!(chat_id.to_string(), "123456");
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
    fn test_session_key_from_parts() {
        let session_key = SessionKey::from_parts("discord", "789");
        assert_eq!(session_key.as_str(), "discord:789");
    }

    #[test]
    fn test_message_role_display() {
        assert_eq!(MessageRole::System.to_string(), "system");
        assert_eq!(MessageRole::User.to_string(), "user");
        assert_eq!(MessageRole::Assistant.to_string(), "assistant");
        assert_eq!(MessageRole::Tool.to_string(), "tool");
    }

    #[test]
    fn test_message_role_from_str() {
        assert_eq!(MessageRole::from("system"), MessageRole::System);
        assert_eq!(MessageRole::from("user"), MessageRole::User);
        assert_eq!(MessageRole::from("assistant"), MessageRole::Assistant);
        assert_eq!(MessageRole::from("tool"), MessageRole::Tool);
        assert_eq!(MessageRole::from("unknown"), MessageRole::User);
    }

    #[test]
    fn test_channel_name_from_string() {
        let channel: ChannelName = "telegram".into();
        assert_eq!(channel.as_str(), "telegram");

        let channel2: ChannelName = String::from("discord").into();
        assert_eq!(channel2.as_str(), "discord");
    }

    #[test]
    fn test_chat_id_from_string() {
        let chat_id: ChatId = "123".into();
        assert_eq!(chat_id.as_str(), "123");

        let chat_id2: ChatId = String::from("456").into();
        assert_eq!(chat_id2.as_str(), "456");
    }

    #[test]
    fn test_session_key_equality() {
        let key1 = SessionKey::from_parts("telegram", "123");
        let key2 = SessionKey::from_parts("telegram", "123");
        let key3 = SessionKey::from_parts("discord", "123");

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_message_role_serialization() {
        let role = MessageRole::User;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"user\"");

        let deserialized: MessageRole = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, MessageRole::User);
    }
}
