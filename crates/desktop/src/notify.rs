//! Tauri-backed notification sender.
//!
//! Routes OS notifications through `tauri-plugin-notification` so that
//! macOS attributes them to the Klynt app and displays the correct app icon.

use common::NotificationSender;
use common::Result;
use tauri_plugin_notification::NotificationExt;

/// Sends notifications via the Tauri notification plugin.
///
/// Unlike the default `osascript` approach (which shows Script Editor's icon),
/// this uses the native notification API through Tauri, so macOS displays the
/// Klynt app icon.
pub struct TauriNotificationSender {
    app_handle: tauri::AppHandle,
}

impl TauriNotificationSender {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self { app_handle }
    }

    /// Send a notification (non-async convenience for focus_timer usage).
    pub fn send_sync(&self, title: &str, body: &str) -> Result<()> {
        self.app_handle
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show()
            .map_err(|e| std::io::Error::other(format!("notification failed: {e}")))?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl NotificationSender for TauriNotificationSender {
    async fn send(&self, title: &str, body: &str) -> Result<()> {
        self.send_sync(title, body)
    }
}
