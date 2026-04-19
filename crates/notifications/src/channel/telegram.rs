//! Telegram notification channel adapter.
//!
//! Wraps a [`channels::TelegramChannel`] (which implements
//! [`channels::Channel`]) and exposes it as a notifications-level [`Channel`].
//! The underlying sender's `send()` method is called with an
//! [`OutboundMessage`](bus::OutboundMessage) targeting the pre-configured chat.

use std::sync::Arc;

use async_trait::async_trait;
use bus::OutboundMessage;
use channels::Channel as ChSend;
use channels::TelegramChannel as TelegramSender;

use crate::error::{NotificationError, Result};

use super::{Channel, NotificationPayload};

/// Delivers notifications to a fixed Telegram chat via an existing
/// [`TelegramChannel`](channels::TelegramChannel) connection.
///
/// Construct by passing an `Arc<TelegramChannel>` (already started / connected)
/// and the numeric Telegram chat ID to target.
pub struct TelegramNotificationChannel {
    sender: Arc<TelegramSender>,
    default_chat_id: i64,
}

impl TelegramNotificationChannel {
    pub fn new(sender: Arc<TelegramSender>, default_chat_id: i64) -> Self {
        Self {
            sender,
            default_chat_id,
        }
    }
}

#[async_trait]
impl Channel for TelegramNotificationChannel {
    fn name(&self) -> &str {
        "telegram"
    }

    async fn deliver(&self, payload: &NotificationPayload) -> Result<()> {
        let text = format!("{}\n{}", payload.title, payload.body);
        let msg = OutboundMessage::new("telegram", self.default_chat_id.to_string(), text);
        self.sender
            .send(&msg)
            .await
            .map_err(|e| NotificationError::Delivery {
                channel: "telegram".into(),
                reason: e.to_string(),
            })
    }
}
