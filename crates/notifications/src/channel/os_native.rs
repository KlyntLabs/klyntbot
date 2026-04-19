//! OS-native desktop notification adapter.
//!
//! Delegates to [`common::NotificationSender`] — the platform-specific
//! implementation is injected at construction time so the notifications
//! crate stays platform-agnostic.

use std::sync::Arc;

use async_trait::async_trait;
use common::NotificationSender;

use crate::error::{NotificationError, Result};

use super::{Channel, NotificationPayload};

/// Wraps an [`Arc<dyn NotificationSender>`] and implements [`Channel`].
pub struct OsNativeChannel {
    sender: Arc<dyn NotificationSender>,
}

impl OsNativeChannel {
    pub fn new(sender: Arc<dyn NotificationSender>) -> Self {
        Self { sender }
    }
}

#[async_trait]
impl Channel for OsNativeChannel {
    fn name(&self) -> &str {
        "os_native"
    }

    async fn deliver(&self, payload: &NotificationPayload) -> Result<()> {
        self.sender
            .send(&payload.title, &payload.body)
            .await
            .map_err(|e| NotificationError::Delivery {
                channel: "os_native".into(),
                reason: e.to_string(),
            })
    }
}
