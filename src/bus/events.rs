//! Event types for the message bus.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Message received from a chat channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    /// Channel name (telegram, discord, slack, whatsapp, etc.)
    pub channel: String,

    /// User identifier
    pub sender_id: String,

    /// Chat/channel identifier
    pub chat_id: String,

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
}

impl InboundMessage {
    /// Create a new inbound message
    pub fn new(
        channel: impl Into<String>,
        sender_id: impl Into<String>,
        chat_id: impl Into<String>,
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
        }
    }

    /// Get the session key for this message (channel:chat_id)
    pub fn session_key(&self) -> String {
        format!("{}:{}", self.channel, self.chat_id)
    }
}

/// Message to send to a chat channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    /// Channel name
    pub channel: String,

    /// Chat/channel identifier
    pub chat_id: String,

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
        channel: impl Into<String>,
        chat_id: impl Into<String>,
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
