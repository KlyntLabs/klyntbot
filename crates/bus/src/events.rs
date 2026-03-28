//! Event types for the message bus.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use common::{ChannelName, ChatId, SessionKey};

/// Maximum message content size (64 KB)
pub const MAX_MESSAGE_SIZE: usize = 65536;

/// Distinguishes text messages from reactions (emoji on a previous message).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MessageKind {
    #[default]
    Text,
    Reaction,
    Voice,
}

/// Message received from a chat channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    /// Channel name (telegram, discord, slack, whatsapp, etc.)
    pub channel: ChannelName,

    /// User identifier
    pub sender_id: String,

    /// Chat/channel identifier
    pub chat_id: ChatId,

    /// Message text content
    pub content: String,

    /// Timestamp of message
    #[serde(default = "Utc::now")]
    pub timestamp: DateTime<Utc>,

    /// Media URLs (images, files, etc.)
    #[serde(default)]
    pub media: Vec<String>,

    /// Channel-specific metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,

    /// Whether this is a text message or a reaction
    #[serde(default)]
    pub kind: MessageKind,
}

impl InboundMessage {
    /// Create a new inbound message
    pub fn new(
        channel: impl Into<ChannelName>,
        sender_id: impl Into<String>,
        chat_id: impl Into<ChatId>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            channel: channel.into(),
            sender_id: sender_id.into(),
            chat_id: chat_id.into(),
            content: content.into(),
            timestamp: Utc::now(),
            media: Vec::new(),
            metadata: HashMap::new(),
            kind: MessageKind::Text,
        }
    }

    /// Set the message kind (text or reaction)
    pub fn with_kind(mut self, kind: MessageKind) -> Self {
        self.kind = kind;
        self
    }

    /// Get the session key for this message (channel:chat_id)
    pub fn session_key(&self) -> SessionKey {
        SessionKey::new(&self.channel, &self.chat_id)
    }

    /// Validate message size. Returns an error string if the message is too large.
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.content.len() > MAX_MESSAGE_SIZE {
            return Err(format!(
                "Message content too large: {} bytes (max {})",
                self.content.len(),
                MAX_MESSAGE_SIZE
            ));
        }
        Ok(())
    }
}

/// Message to send to a chat channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    /// Channel name
    pub channel: ChannelName,

    /// Chat/channel identifier
    pub chat_id: ChatId,

    /// Message text content
    pub content: String,

    /// Optional message ID to reply to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,

    /// Media URLs to send
    #[serde(default)]
    pub media: Vec<String>,

    /// Channel-specific metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl OutboundMessage {
    /// Create a new outbound message
    pub fn new(
        channel: impl Into<ChannelName>,
        chat_id: impl Into<ChatId>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            channel: channel.into(),
            chat_id: chat_id.into(),
            content: content.into(),
            reply_to: None,
            media: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Set reply_to message ID
    pub fn with_reply_to(mut self, message_id: impl Into<String>) -> Self {
        self.reply_to = Some(message_id.into());
        self
    }

    /// Add media URL
    pub fn with_media(mut self, media_url: impl Into<String>) -> Self {
        self.media.push(media_url.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inbound_message_default_kind_is_text() {
        let msg = InboundMessage::new("telegram", "user1", "chat1", "hello");
        assert!(matches!(msg.kind, MessageKind::Text));
    }

    #[test]
    fn test_inbound_message_with_reaction_kind() {
        let msg = InboundMessage::new("telegram", "user1", "chat1", "\u{1F44D}")
            .with_kind(MessageKind::Reaction);
        assert!(matches!(msg.kind, MessageKind::Reaction));
        assert_eq!(msg.content, "\u{1F44D}");
    }

    #[test]
    fn test_message_kind_default_trait() {
        let kind = MessageKind::default();
        assert!(matches!(kind, MessageKind::Text));
    }
}
