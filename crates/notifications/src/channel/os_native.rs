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
        let is_urgent = payload.priority_override.as_deref() == Some("urgent");
        let result = if is_urgent {
            self.sender
                .send_critical(&payload.title, &payload.body)
                .await
        } else {
            self.sender.send(&payload.title, &payload.body).await
        };
        result.map_err(|e| NotificationError::Delivery {
            channel: "os_native".into(),
            reason: e.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;
    use crate::channel::NotificationPayload;

    struct MockSender {
        send_count: AtomicU32,
        critical_count: AtomicU32,
    }

    impl MockSender {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                send_count: AtomicU32::new(0),
                critical_count: AtomicU32::new(0),
            })
        }
    }

    #[async_trait]
    impl NotificationSender for MockSender {
        async fn send(&self, _title: &str, _body: &str) -> common::Result<()> {
            self.send_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn send_critical(&self, _title: &str, _body: &str) -> common::Result<()> {
            self.critical_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    fn make_payload(priority_override: Option<&str>) -> NotificationPayload {
        NotificationPayload {
            alarm_id: "test-alarm".into(),
            title: "Test".into(),
            body: "Body".into(),
            priority: crate::channel::Priority::Normal,
            channel_mask: 0,
            priority_override: priority_override.map(str::to_owned),
        }
    }

    #[tokio::test]
    async fn urgent_priority_invokes_send_critical() {
        let mock = MockSender::new();
        let channel = OsNativeChannel::new(mock.clone());

        channel
            .deliver(&make_payload(Some("urgent")))
            .await
            .unwrap();

        assert_eq!(
            mock.critical_count.load(Ordering::SeqCst),
            1,
            "send_critical must be called once"
        );
        assert_eq!(
            mock.send_count.load(Ordering::SeqCst),
            0,
            "send must not be called"
        );
    }

    #[tokio::test]
    async fn non_urgent_priority_invokes_send() {
        let mock = MockSender::new();
        let channel = OsNativeChannel::new(mock.clone());

        channel.deliver(&make_payload(None)).await.unwrap();

        assert_eq!(
            mock.send_count.load(Ordering::SeqCst),
            1,
            "send must be called once"
        );
        assert_eq!(
            mock.critical_count.load(Ordering::SeqCst),
            0,
            "send_critical must not be called"
        );
    }
}
