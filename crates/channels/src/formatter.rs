//! Channel-specific markdown formatting/normalization.
//!
//! Each channel has its own formatting requirements:
//! - Telegram: markdown -> HTML
//! - Discord: passthrough (native markdown support)
//! - Slack: standard markdown -> Slack mrkdwn
//! - Email: strip markdown to plain text

use regex::Regex;
use std::sync::OnceLock;

/// Trait for channel-specific message formatting.
pub trait ChannelFormatter: Send + Sync {
    fn format(&self, markdown: &str) -> String;
}

/// Returns the appropriate formatter for a channel name.
pub fn formatter_for(channel: &str) -> &'static dyn ChannelFormatter {
    static TELEGRAM: TelegramFormatter = TelegramFormatter;
    static PASSTHROUGH: PassthroughFormatter = PassthroughFormatter;
    static SLACK: SlackFormatter = SlackFormatter;
    static PLAIN: PlainTextFormatter = PlainTextFormatter;

    match channel {
        "telegram" => &TELEGRAM,
        "discord" => &PASSTHROUGH,
        "slack" => &SLACK,
        "email" => &PLAIN,
        _ => &PLAIN,
    }
}

// ---------------------------------------------------------------------------
// Telegram: markdown -> HTML
// ---------------------------------------------------------------------------

struct MarkdownRegexes {
    code_block: Regex,
    inline_code: Regex,
    header: Regex,
    blockquote: Regex,
    link: Regex,
    bold_star: Regex,
    bold_underscore: Regex,
    italic: Regex,
    strikethrough: Regex,
    bullet: Regex,
}

fn markdown_regexes() -> &'static MarkdownRegexes {
    static REGEXES: OnceLock<MarkdownRegexes> = OnceLock::new();
    REGEXES.get_or_init(|| MarkdownRegexes {
        code_block: Regex::new(r"```[\w]*\n?([\s\S]*?)```").unwrap(),
        inline_code: Regex::new(r"`([^`]+)`").unwrap(),
        header: Regex::new(r"(?m)^#{1,6}\s+(.+)$").unwrap(),
        blockquote: Regex::new(r"(?m)^>\s*(.*)$").unwrap(),
        link: Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap(),
        bold_star: Regex::new(r"\*\*(.+?)\*\*").unwrap(),
        bold_underscore: Regex::new(r"__(.+?)__").unwrap(),
        italic: Regex::new(r"(?:^|(?P<pre>[^a-zA-Z0-9]))_([^_]+)_(?:$|(?P<post>[^a-zA-Z0-9]))")
            .unwrap(),
        strikethrough: Regex::new(r"~~(.+?)~~").unwrap(),
        bullet: Regex::new(r"(?m)^[-*]\s+").unwrap(),
    })
}

pub struct TelegramFormatter;

impl ChannelFormatter for TelegramFormatter {
    fn format(&self, text: &str) -> String {
        if text.is_empty() {
            return String::new();
        }

        let re = markdown_regexes();
        let mut result = text.to_string();

        // Use a unique sentinel prefix that cannot appear in user input.
        // We use a fixed high-Unicode sequence unlikely in real text.
        let sentinel = "\u{FEFF}\u{200B}\u{FEFF}";

        // 1. Extract and protect code blocks
        let mut code_blocks: Vec<String> = Vec::new();
        result = re
            .code_block
            .replace_all(&result, |caps: &regex::Captures| {
                code_blocks.push(caps.get(1).unwrap().as_str().to_string());
                format!("{}CB{}{}", sentinel, code_blocks.len() - 1, sentinel)
            })
            .to_string();

        // 2. Extract and protect inline code
        let mut inline_codes: Vec<String> = Vec::new();
        result = re
            .inline_code
            .replace_all(&result, |caps: &regex::Captures| {
                inline_codes.push(caps.get(1).unwrap().as_str().to_string());
                format!("{}IC{}{}", sentinel, inline_codes.len() - 1, sentinel)
            })
            .to_string();

        // 3. Headers -> just text
        result = re.header.replace_all(&result, "$1").to_string();

        // 4. Blockquotes -> just text
        result = re.blockquote.replace_all(&result, "$1").to_string();

        // 5. Escape HTML
        result = result
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");

        // 6. Links (escape " in URLs to prevent attribute injection)
        result = re
            .link
            .replace_all(&result, |caps: &regex::Captures| {
                let text = caps.get(1).unwrap().as_str();
                let url = caps.get(2).unwrap().as_str().replace('"', "&quot;");
                format!(r#"<a href="{}">{}</a>"#, url, text)
            })
            .to_string();

        // 7. Bold
        result = re.bold_star.replace_all(&result, "<b>$1</b>").to_string();
        result = re
            .bold_underscore
            .replace_all(&result, "<b>$1</b>")
            .to_string();

        // 8. Italic
        result = re
            .italic
            .replace_all(&result, "${pre}<i>$2</i>${post}")
            .to_string();

        // 9. Strikethrough
        result = re
            .strikethrough
            .replace_all(&result, "<s>$1</s>")
            .to_string();

        // 10. Bullets
        result = re.bullet.replace_all(&result, "• ").to_string();

        // 11. Restore inline code
        for (i, code) in inline_codes.iter().enumerate() {
            let escaped = code
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            result = result.replace(
                &format!("{}IC{}{}", sentinel, i, sentinel),
                &format!("<code>{}</code>", escaped),
            );
        }

        // 12. Restore code blocks
        for (i, code) in code_blocks.iter().enumerate() {
            let escaped = code
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            result = result.replace(
                &format!("{}CB{}{}", sentinel, i, sentinel),
                &format!("<pre><code>{}</code></pre>", escaped),
            );
        }

        result
    }
}

// ---------------------------------------------------------------------------
// Passthrough: no-op (Discord supports markdown natively)
// ---------------------------------------------------------------------------

pub struct PassthroughFormatter;

impl ChannelFormatter for PassthroughFormatter {
    fn format(&self, markdown: &str) -> String {
        markdown.to_string()
    }
}

// ---------------------------------------------------------------------------
// Slack: standard markdown -> Slack mrkdwn
// ---------------------------------------------------------------------------

struct SlackRegexes {
    bold: Regex,
    strikethrough: Regex,
    link: Regex,
    header: Regex,
    code_block: Regex,
}

fn slack_regexes() -> &'static SlackRegexes {
    static REGEXES: OnceLock<SlackRegexes> = OnceLock::new();
    REGEXES.get_or_init(|| SlackRegexes {
        bold: Regex::new(r"\*\*(.+?)\*\*").unwrap(),
        strikethrough: Regex::new(r"~~(.+?)~~").unwrap(),
        link: Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap(),
        header: Regex::new(r"(?m)^#{1,6}\s+(.+)$").unwrap(),
        code_block: Regex::new(r"```\w*\n?").unwrap(),
    })
}

pub struct SlackFormatter;

impl ChannelFormatter for SlackFormatter {
    fn format(&self, text: &str) -> String {
        if text.is_empty() {
            return String::new();
        }

        let re = slack_regexes();
        let mut result = text.to_string();

        // Headers -> *bold text*
        result = re.header.replace_all(&result, "*$1*").to_string();

        // Bold **text** -> *text*
        result = re.bold.replace_all(&result, "*$1*").to_string();

        // Italic _text_ stays as _text_ in Slack mrkdwn (compatible)

        // Strikethrough ~~text~~ -> ~text~
        result = re.strikethrough.replace_all(&result, "~$1~").to_string();

        // Links [text](url) -> <url|text>
        result = re.link.replace_all(&result, "<$2|$1>").to_string();

        // Code blocks: strip language identifier from ```lang -> ```
        result = re.code_block.replace_all(&result, "```\n").to_string();

        result
    }
}

// ---------------------------------------------------------------------------
// PlainText: strip all markdown (Email, unknown channels)
// ---------------------------------------------------------------------------

struct PlainRegexes {
    code_block: Regex,
    inline_code: Regex,
    bold_star: Regex,
    bold_underscore: Regex,
    italic: Regex,
    strikethrough: Regex,
    link: Regex,
    header: Regex,
    blockquote: Regex,
    bullet: Regex,
}

fn plain_regexes() -> &'static PlainRegexes {
    static REGEXES: OnceLock<PlainRegexes> = OnceLock::new();
    REGEXES.get_or_init(|| PlainRegexes {
        code_block: Regex::new(r"```[\w]*\n?([\s\S]*?)```").unwrap(),
        inline_code: Regex::new(r"`([^`]+)`").unwrap(),
        bold_star: Regex::new(r"\*\*(.+?)\*\*").unwrap(),
        bold_underscore: Regex::new(r"__(.+?)__").unwrap(),
        italic: Regex::new(r"(?:^|(?P<pre>[^a-zA-Z0-9]))_([^_]+)_(?:$|(?P<post>[^a-zA-Z0-9]))")
            .unwrap(),
        strikethrough: Regex::new(r"~~(.+?)~~").unwrap(),
        link: Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap(),
        header: Regex::new(r"(?m)^#{1,6}\s+(.+)$").unwrap(),
        blockquote: Regex::new(r"(?m)^>\s*(.*)$").unwrap(),
        bullet: Regex::new(r"(?m)^[-*]\s+").unwrap(),
    })
}

pub struct PlainTextFormatter;

impl ChannelFormatter for PlainTextFormatter {
    fn format(&self, text: &str) -> String {
        if text.is_empty() {
            return String::new();
        }

        let re = plain_regexes();
        let mut result = text.to_string();

        // Code blocks -> just the code content
        result = re.code_block.replace_all(&result, "$1").to_string();

        // Inline code -> just the content
        result = re.inline_code.replace_all(&result, "$1").to_string();

        // Headers -> just text
        result = re.header.replace_all(&result, "$1").to_string();

        // Blockquotes -> just text
        result = re.blockquote.replace_all(&result, "$1").to_string();

        // Bold -> just text
        result = re.bold_star.replace_all(&result, "$1").to_string();
        result = re.bold_underscore.replace_all(&result, "$1").to_string();

        // Italic -> just text
        result = re
            .italic
            .replace_all(&result, "${pre}$2${post}")
            .to_string();

        // Strikethrough -> just text
        result = re.strikethrough.replace_all(&result, "$1").to_string();

        // Links -> "text (url)"
        result = re.link.replace_all(&result, "$1 ($2)").to_string();

        // Bullets -> "- " (normalized)
        result = re.bullet.replace_all(&result, "- ").to_string();

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- TelegramFormatter tests --

    #[test]
    fn test_telegram_code_blocks() {
        let f = TelegramFormatter;
        let out = f.format("```rust\nfn main() {}\n```");
        assert!(out.contains("<pre><code>"));
        assert!(out.contains("fn main()"));
    }

    #[test]
    fn test_telegram_inline_code() {
        let f = TelegramFormatter;
        let out = f.format("Use `println!` here");
        assert!(out.contains("<code>println!</code>"));
    }

    #[test]
    fn test_telegram_bold() {
        let f = TelegramFormatter;
        let out = f.format("This is **bold**");
        assert!(out.contains("<b>bold</b>"));
    }

    #[test]
    fn test_telegram_link() {
        let f = TelegramFormatter;
        let out = f.format("[Rust](https://rust-lang.org)");
        assert!(out.contains(r#"<a href="https://rust-lang.org">Rust</a>"#));
    }

    #[test]
    fn test_telegram_link_injection_prevented() {
        let f = TelegramFormatter;
        let malicious = r#"[click](https://evil.com" onmouseover="alert(1))"#;
        let out = f.format(malicious);
        // Must not contain unescaped " in href attribute
        assert!(!out.contains(r#"" onmouseover"#));
        assert!(out.contains("&quot;"));
    }

    #[test]
    fn test_telegram_html_escape() {
        let f = TelegramFormatter;
        let out = f.format("x < y & z > w");
        assert!(out.contains("&lt;"));
        assert!(out.contains("&amp;"));
        assert!(out.contains("&gt;"));
    }

    #[test]
    fn test_telegram_empty() {
        let f = TelegramFormatter;
        assert_eq!(f.format(""), "");
    }

    // -- PassthroughFormatter tests --

    #[test]
    fn test_passthrough() {
        let f = PassthroughFormatter;
        let input = "**bold** _italic_ `code`";
        assert_eq!(f.format(input), input);
    }

    // -- SlackFormatter tests --

    #[test]
    fn test_slack_bold() {
        let f = SlackFormatter;
        let out = f.format("This is **bold** text");
        assert!(out.contains("*bold*"));
    }

    #[test]
    fn test_slack_strikethrough() {
        let f = SlackFormatter;
        let out = f.format("~~deleted~~");
        assert!(out.contains("~deleted~"));
    }

    #[test]
    fn test_slack_link() {
        let f = SlackFormatter;
        let out = f.format("[Rust](https://rust-lang.org)");
        assert!(out.contains("<https://rust-lang.org|Rust>"));
    }

    #[test]
    fn test_slack_header() {
        let f = SlackFormatter;
        let out = f.format("# Title");
        assert!(out.contains("*Title*"));
    }

    #[test]
    fn test_slack_empty() {
        let f = SlackFormatter;
        assert_eq!(f.format(""), "");
    }

    // -- PlainTextFormatter tests --

    #[test]
    fn test_plain_strips_bold() {
        let f = PlainTextFormatter;
        let out = f.format("**bold** text");
        assert_eq!(out, "bold text");
    }

    #[test]
    fn test_plain_strips_code_block() {
        let f = PlainTextFormatter;
        let out = f.format("```rust\nfn main() {}\n```");
        assert!(out.contains("fn main()"));
        assert!(!out.contains("```"));
    }

    #[test]
    fn test_plain_strips_inline_code() {
        let f = PlainTextFormatter;
        let out = f.format("Use `println!` here");
        assert_eq!(out, "Use println! here");
    }

    #[test]
    fn test_plain_link_to_text() {
        let f = PlainTextFormatter;
        let out = f.format("[Rust](https://rust-lang.org)");
        assert_eq!(out, "Rust (https://rust-lang.org)");
    }

    #[test]
    fn test_plain_header() {
        let f = PlainTextFormatter;
        let out = f.format("## Section Title");
        assert_eq!(out, "Section Title");
    }

    #[test]
    fn test_plain_empty() {
        let f = PlainTextFormatter;
        assert_eq!(f.format(""), "");
    }

    // -- formatter_for tests --

    #[test]
    fn test_formatter_for_channels() {
        // Just verify it doesn't panic and returns reasonable output
        let input = "**bold** `code`";
        assert!(formatter_for("telegram").format(input).contains("<b>"));
        assert_eq!(formatter_for("discord").format(input), input);
        assert!(formatter_for("slack").format(input).contains("*bold*"));
        assert!(!formatter_for("email").format(input).contains("**"));
    }
}
