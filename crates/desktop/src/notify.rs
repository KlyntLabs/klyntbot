//! Tauri-backed notification sender.
//!
//! Routes OS notifications through `tauri-plugin-notification` so that
//! macOS attributes them to the Klynt app and displays the correct app icon.
//! When the Tauri notification channel fails, there is no fallback on this
//! platform.

use common::NotificationSender;
use common::Result;
use tauri_plugin_notification::NotificationExt;

/// Title shown when a focus work block completes.
pub const FOCUS_WORK_COMPLETE_TITLE: &str = "Focus Session Complete";

/// Body shown when a focus work block completes.
pub fn focus_work_complete_body(duration_mins: u64) -> String {
    format!("{duration_mins}m focus session complete. Break time!")
}

/// Title shown when a break block completes.
pub const FOCUS_BREAK_COMPLETE_TITLE: &str = "Break Over";

/// Body shown when a break block completes.
pub const FOCUS_BREAK_COMPLETE_BODY: &str = "Ready for the next focus session!";

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

    /// Send a notification, falling back to a platform helper if Tauri fails.
    ///
    /// The fallback is best-effort: currently no explicit fallback is
    /// implemented because the Tauri plugin covers macOS natively.
    pub fn send_sync_with_fallback(&self, title: &str, body: &str) -> Result<()> {
        match self.send_sync(title, body) {
            Ok(()) => Ok(()),
            Err(e) => {
                ::tracing::warn!("Tauri notification failed ({e}); trying platform fallback");
                platform_fallback_notify(title, body).map_err(|f| {
                    common::KlyntbotError::Io(std::io::Error::other(format!(
                        "notification failed: {e}; fallback failed: {f}"
                    )))
                })?;
                Ok(())
            }
        }
    }

    /// Notify the user that a focus work block has completed.
    pub fn send_focus_work_complete(&self, duration_mins: u64) -> Result<()> {
        self.send_sync_with_fallback(
            FOCUS_WORK_COMPLETE_TITLE,
            &focus_work_complete_body(duration_mins),
        )
    }

    /// Notify the user that a break block has completed.
    pub fn send_focus_break_complete(&self) -> Result<()> {
        self.send_sync_with_fallback(FOCUS_BREAK_COMPLETE_TITLE, FOCUS_BREAK_COMPLETE_BODY)
    }
}

fn platform_fallback_notify(_title: &str, _body: &str) -> Result<()> {
    Err(common::KlyntbotError::Io(std::io::Error::other(
        "no platform notification fallback available",
    )))
}

#[async_trait::async_trait]
impl NotificationSender for TauriNotificationSender {
    async fn send(&self, title: &str, body: &str) -> Result<()> {
        self.send_sync(title, body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_work_complete_body_format() {
        assert_eq!(
            focus_work_complete_body(25),
            "25m focus session complete. Break time!"
        );
    }

    #[test]
    fn fallback_args_match_content() {
        // This test guards the contract used by the platform fallback so that
        // future i18n changes do not drop content.
        let title = FOCUS_WORK_COMPLETE_TITLE;
        let body = focus_work_complete_body(25);
        assert!(title.contains("Focus"));
        assert!(body.contains("25"));
        assert!(body.contains("Break"));
    }
}
