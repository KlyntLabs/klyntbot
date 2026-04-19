//! Notification delivery port.
//!
//! Implement [`NotificationSender`] in higher layers (e.g. the desktop crate)
//! to route notifications through a framework-specific API.

use crate::Result;

/// Abstraction over OS notification delivery.
#[async_trait::async_trait]
pub trait NotificationSender: Send + Sync {
    async fn send(&self, title: &str, body: &str) -> Result<()>;

    /// Deliver with elevated urgency. Best-effort per platform:
    /// - Linux: `notify-send -u critical` (bypasses DND on most daemons).
    /// - macOS: audible cue + "URGENT" prefix (true Focus bypass requires entitlements).
    /// - Windows: toast with alarm audio priority.
    /// Default impl delegates to `send`.
    async fn send_critical(&self, title: &str, body: &str) -> Result<()> {
        self.send(title, body).await
    }
}
