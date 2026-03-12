//! Notification delivery port.
//!
//! Implement [`NotificationSender`] in higher layers (e.g. the desktop crate)
//! to route notifications through a framework-specific API.

use crate::Result;

/// Abstraction over OS notification delivery.
#[async_trait::async_trait]
pub trait NotificationSender: Send + Sync {
    async fn send(&self, title: &str, body: &str) -> Result<()>;
}
