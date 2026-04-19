//! Discord notification channel adapter.
//!
//! Wraps a [`channels::DiscordChannel`] (which implements
//! [`channels::Channel`]) and exposes it as a notifications-level [`Channel`].
//! The underlying sender's `send()` method is called with an
//! [`OutboundMessage`](bus::OutboundMessage) targeting the pre-configured guild
//! channel.

use std::sync::Arc;

use async_trait::async_trait;
use bus::OutboundMessage;
use channels::Channel as ChSend;
use channels::DiscordChannel as DiscordSender;

use crate::error::{NotificationError, Result};

use super::{Channel, NotificationPayload};

/// Delivers notifications to a fixed Discord channel via an existing
/// [`DiscordChannel`](channels::DiscordChannel) connection.
///
/// Construct by passing an `Arc<DiscordChannel>` (already started / connected)
/// and the Discord channel ID (as a string) to target.
pub struct DiscordNotificationChannel {
    sender: Arc<DiscordSender>,
    default_channel_id: String,
}

impl DiscordNotificationChannel {
    pub fn new(sender: Arc<DiscordSender>, default_channel_id: impl Into<String>) -> Self {
        Self {
            sender,
            default_channel_id: default_channel_id.into(),
        }
    }
}

#[async_trait]
impl Channel for DiscordNotificationChannel {
    fn name(&self) -> &str {
        "discord"
    }

    async fn deliver(&self, payload: &NotificationPayload) -> Result<()> {
        // Discord uses Markdown natively.
        let content = format!("**{}**\n{}", payload.title, payload.body);
        let msg = OutboundMessage::new("discord", self.default_channel_id.as_str(), content);
        self.sender
            .send(&msg)
            .await
            .map_err(|e| NotificationError::Delivery {
                channel: "discord".into(),
                reason: e.to_string(),
            })
    }
}
