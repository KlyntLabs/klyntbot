//! Channel-aware response formatting.
//!
//! Adapts LLM response content for each chat platform's constraints:
//! - Telegram: preserve markdown, limit 4096 chars
//! - Discord: code blocks supported, limit 2000 chars
//! - WhatsApp: strip markdown, use emojis for emphasis
//! - Default/CLI: pass through unchanged

use common::utils::truncate_at_boundary;
use common::ChannelName;

/// Maximum message length per channel.
const TELEGRAM_MAX_CHARS: usize = 4096;
const DISCORD_MAX_CHARS: usize = 2000;
const WHATSAPP_MAX_CHARS: usize = 4000;

/// Format content for a specific channel's constraints.
pub fn format_for_channel(content: &str, channel: &ChannelName) -> String {
    match channel.as_str() {
        "telegram" => format_telegram(content),
        "discord" => format_discord(content),
        "whatsapp" => format_whatsapp(content),
        _ => content.to_string(), // CLI and others: pass through
    }
}

fn format_telegram(content: &str) -> String {
    // Telegram supports markdown — preserve it, just truncate if needed
    truncate_with_ellipsis(content, TELEGRAM_MAX_CHARS)
}

fn format_discord(content: &str) -> String {
    // Discord supports markdown and code blocks — preserve them
    truncate_with_ellipsis(content, DISCORD_MAX_CHARS)
}

fn format_whatsapp(content: &str) -> String {
    // WhatsApp: strip markdown formatting, use emojis for emphasis
    let stripped = strip_markdown(content);
    truncate_with_ellipsis(&stripped, WHATSAPP_MAX_CHARS)
}

/// Strip basic markdown formatting characters.
fn strip_markdown(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            // Bold **text** or __text__ → text
            '*' | '_' => {
                if chars.peek() == Some(&ch) {
                    chars.next(); // skip second marker
                }
                // Skip the marker itself
            }
            // Inline code `text` → text
            '`' => {
                // Skip backticks (including triple ```)
                while chars.peek() == Some(&'`') {
                    chars.next();
                }
            }
            // Headers # → skip the # and following space
            '#' if result.is_empty() || result.ends_with('\n') => {
                while chars.peek() == Some(&'#') {
                    chars.next();
                }
                if chars.peek() == Some(&' ') {
                    chars.next();
                }
            }
            _ => result.push(ch),
        }
    }

    result
}

/// Truncate content at a word boundary, appending "..." if truncated.
/// Delegates UTF-8 boundary safety to `common::utils::truncate_at_boundary`.
fn truncate_with_ellipsis(content: &str, max_bytes: usize) -> String {
    if content.len() <= max_bytes {
        return content.to_string();
    }

    // Leave room for "..."
    let limit = max_bytes.saturating_sub(3);

    // Get a UTF-8-safe slice via the common utility
    let safe_slice = truncate_at_boundary(content, limit);

    // Find last whitespace for a clean word break
    let cut_point = safe_slice
        .rfind(char::is_whitespace)
        .unwrap_or(safe_slice.len());

    format!("{}...", &content[..cut_point])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_passthrough() {
        let content = "Hello **world**";
        let result = format_for_channel(content, &ChannelName::new("cli"));
        assert_eq!(result, content);
    }

    #[test]
    fn test_telegram_preserves_markdown() {
        let content = "Hello **world** `code`";
        let result = format_for_channel(content, &ChannelName::new("telegram"));
        assert_eq!(result, content);
    }

    #[test]
    fn test_telegram_truncates_long_content() {
        let long = "a ".repeat(3000); // 6000 chars
        let result = format_for_channel(&long, &ChannelName::new("telegram"));
        assert!(result.len() <= TELEGRAM_MAX_CHARS);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_discord_truncates_at_2000() {
        let long = "word ".repeat(500); // 2500 chars
        let result = format_for_channel(&long, &ChannelName::new("discord"));
        assert!(result.len() <= DISCORD_MAX_CHARS);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_whatsapp_strips_markdown() {
        let content = "**Bold** and `code` and _italic_";
        let result = format_for_channel(content, &ChannelName::new("whatsapp"));
        assert!(!result.contains("**"));
        assert!(!result.contains('`'));
        assert!(result.contains("Bold"));
        assert!(result.contains("code"));
    }

    #[test]
    fn test_strip_markdown_headers() {
        let content = "## Header\nSome text";
        let stripped = strip_markdown(content);
        assert!(!stripped.contains('#'));
        assert!(stripped.contains("Header"));
        assert!(stripped.contains("Some text"));
    }

    #[test]
    fn test_truncate_short_content() {
        let content = "short";
        assert_eq!(truncate_with_ellipsis(content, 100), "short");
    }

    #[test]
    fn test_truncate_at_word_boundary() {
        let content = "hello world this is a test";
        let result = truncate_with_ellipsis(content, 16);
        // Should cut before "this" and add "..."
        assert!(result.ends_with("..."));
        assert!(result.len() <= 16);
    }

    #[test]
    fn test_unknown_channel_passes_through() {
        let content = "**markdown** content";
        let result = format_for_channel(content, &ChannelName::new("qq"));
        assert_eq!(result, content);
    }
}
