//! Email notification channel adapter.
//!
//! Wraps a [`channels::EmailChannel`] (which implements [`channels::Channel`])
//! and exposes it as a notifications-level [`Channel`].  The underlying
//! sender's `send()` method is called with an
//! [`OutboundMessage`](bus::OutboundMessage) addressed to the pre-configured
//! recipient.

use std::sync::Arc;

use async_trait::async_trait;
use bus::OutboundMessage;
use channels::Channel as ChSend;

#[cfg(feature = "email")]
use channels::EmailChannel as EmailSender;

use crate::error::{NotificationError, Result};

use super::{Channel, NotificationPayload};

/// Delivers notifications via SMTP through an existing
/// [`EmailChannel`](channels::EmailChannel) connection.
///
/// The `title` field of the payload is used as the email subject; `body` as
/// the message body.
///
/// Only available when the `email` feature is enabled (default).
#[cfg(feature = "email")]
pub struct EmailNotificationChannel {
    sender: Arc<EmailSender>,
    default_recipient: String,
}

#[cfg(feature = "email")]
impl EmailNotificationChannel {
    pub fn new(sender: Arc<EmailSender>, default_recipient: impl Into<String>) -> Self {
        Self {
            sender,
            default_recipient: default_recipient.into(),
        }
    }
}

#[cfg(feature = "email")]
#[async_trait]
impl Channel for EmailNotificationChannel {
    fn name(&self) -> &str {
        "email"
    }

    async fn deliver(&self, payload: &NotificationPayload) -> Result<()> {
        // The email adapter's OutboundMessage content is interpreted as the
        // reply body; the subject is derived from last_subject_by_chat inside
        // EmailChannel. For fresh notification emails we construct a simple
        // content string that doubles as subject context.
        let content = format!("{}: {}", payload.title, payload.body);
        let msg = OutboundMessage::new("email", self.default_recipient.as_str(), content);
        self.sender
            .send(&msg)
            .await
            .map_err(|e| NotificationError::Delivery {
                channel: "email".into(),
                reason: e.to_string(),
            })
    }
}
