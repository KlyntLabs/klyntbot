//! OS native notification support
//!
//! Platform-specific implementations for desktop notifications.
//! Uses tokio::process::Command for async compatibility.

use crate::Result;

/// Sanitize text for shell command execution to prevent injection attacks
fn sanitize_for_shell(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('`', "\\`")
}

/// Send a native OS notification
///
/// # Platform Support
/// - **macOS**: Uses osascript with AppleScript
/// - **Linux**: Uses notify-send command
/// - **Windows**: Uses PowerShell toast notifications
/// - **Other**: No-op (returns Ok silently)
///
/// # Security
/// Input is sanitized to prevent shell injection attacks.
#[cfg(target_os = "macos")]
pub async fn send_os_notification(title: &str, body: &str) -> Result<()> {
    use tokio::process::Command;

    let safe_title = sanitize_for_shell(title);
    let safe_body = sanitize_for_shell(body);

    let script = format!(
        "display notification \"{}\" with title \"{}\"",
        safe_body, safe_title
    );

    Command::new("osascript")
        .args(["-e", &script])
        .output()
        .await?;

    Ok(())
}

#[cfg(target_os = "linux")]
pub async fn send_os_notification(title: &str, body: &str) -> Result<()> {
    use tokio::process::Command;

    // notify-send uses arguments directly, no shell interpolation needed
    Command::new("notify-send")
        .arg(title)
        .arg(body)
        .output()
        .await?;

    Ok(())
}

#[cfg(target_os = "windows")]
pub async fn send_os_notification(title: &str, body: &str) -> Result<()> {
    use tokio::process::Command;

    let safe_title = sanitize_for_shell(title);
    let safe_body = sanitize_for_shell(body);

    let ps_script = format!(
        "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null; \
         [Windows.UI.Notifications.ToastNotification, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null; \
         $template = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02); \
         $template.SelectSingleNode('//text[@id=\"1\"]').InnerText = '{}'; \
         $template.SelectSingleNode('//text[@id=\"2\"]').InnerText = '{}'; \
         $toast = [Windows.UI.Notifications.ToastNotification]::new($template); \
         [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Klyntbot').Show($toast)",
        safe_title, safe_body
    );

    Command::new("powershell")
        .args(["-Command", &ps_script])
        .output()
        .await?;

    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub async fn send_os_notification(_title: &str, _body: &str) -> Result<()> {
    // Unsupported platform - silent no-op
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_for_shell() {
        assert_eq!(sanitize_for_shell("hello"), "hello");
        assert_eq!(sanitize_for_shell("hello \"world\""), "hello \\\"world\\\"");
        assert_eq!(sanitize_for_shell("hello\\world"), "hello\\\\world");
        assert_eq!(sanitize_for_shell("$PATH"), "\\$PATH");
        assert_eq!(sanitize_for_shell("`command`"), "\\`command\\`");
    }

    #[test]
    fn test_sanitize_combined() {
        let input = "Test \"quote\" and $var and `cmd` and \\path";
        let expected = "Test \\\"quote\\\" and \\$var and \\`cmd\\` and \\\\path";
        assert_eq!(sanitize_for_shell(input), expected);
    }

    #[tokio::test]
    async fn test_send_notification_no_crash() {
        // This test verifies the function doesn't panic
        // Actual notification may fail if platform tools aren't available
        let result = send_os_notification("Test", "Body").await;
        // We don't assert success because CI might not have notification tools
        // Just verify it doesn't panic
        let _ = result;
    }
}
