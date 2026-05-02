//! Structured truncation policies for tool results and exec output.
//! Ported from codex `utils/output-truncation`.

use serde::{Deserialize, Serialize};

/// How to budget a truncation operation.
///
/// `Bytes(n)` — keep at most `n` bytes (UTF-8 safe, middle-truncated).
/// `Tokens(n)` — keep at most `n` approximate tokens (4 chars ≈ 1 token).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TruncationPolicy {
    Bytes(usize),
    Tokens(usize),
}

impl TruncationPolicy {
    /// Bytes-equivalent of this budget. Tokens are approximated at 4 bytes/token.
    pub fn byte_budget(self) -> usize {
        match self {
            Self::Bytes(b) => b,
            Self::Tokens(t) => t.saturating_mul(4),
        }
    }

    /// Tokens-equivalent of this budget. Bytes are approximated at 4 bytes/token.
    pub fn token_budget(self) -> usize {
        match self {
            Self::Bytes(b) => b / 4,
            Self::Tokens(t) => t,
        }
    }
}

/// Truncate `content` to at most `byte_budget` bytes by keeping the first and
/// last halves and replacing the middle with `[...] omitted N bytes [...]`.
/// UTF-8 safe — never splits a multibyte char.
pub fn truncate_middle_chars(content: &str, byte_budget: usize) -> String {
    if content.len() <= byte_budget {
        return content.to_string();
    }

    const MARKER_TEMPLATE: &str = "\n\n[...] omitted XXXXXXXXXX bytes [...]\n\n";
    let marker_overhead = MARKER_TEMPLATE.len();

    if byte_budget <= marker_overhead {
        let omitted = content.len();
        return format!("\n[...] omitted {omitted} bytes [...]\n");
    }

    let visible = byte_budget - marker_overhead;
    let head_target = visible / 2;
    let tail_target = visible - head_target;

    // Find safe char boundaries.
    let head_end = content.floor_char_boundary(head_target);
    let tail_start = content.floor_char_boundary(content.len().saturating_sub(tail_target));
    if tail_start <= head_end {
        // Pathological case — fall back to head only.
        return content[..head_end].to_string();
    }

    let omitted = tail_start - head_end;
    format!(
        "{}\n\n[...] omitted {} bytes [...]\n\n{}",
        &content[..head_end],
        omitted,
        &content[tail_start..]
    )
}

/// Truncate `content` per `policy`. If truncated, prepends a
/// `"Total output lines: N\n\n"` header so the model knows how much was cut.
pub fn formatted_truncate_text(content: &str, policy: TruncationPolicy) -> String {
    if content.len() <= policy.byte_budget() {
        return content.to_string();
    }
    let total_lines = content.lines().count();
    let truncated = truncate_middle_chars(content, policy.byte_budget());
    format!("Total output lines: {total_lines}\n\n{truncated}")
}

/// Truncate without the "Total output lines:" prefix. Use this for non-tool
/// transport-layer caps (WebSocket payloads etc.) where the model never sees
/// the result.
pub fn truncate_text(content: &str, policy: TruncationPolicy) -> String {
    truncate_middle_chars(content, policy.byte_budget())
}

/// A single item inside a tool result. Mirrors codex's
/// `FunctionCallOutputContentItem` but stripped to the variants Klynt needs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentItem {
    Text(String),
    Image { url: String },
}

/// Truncate a list of content items per `policy`. Images always survive.
/// Text items are truncated middle-char-wise, distributed across the budget
/// in order. When a text item runs out of budget it's dropped and a final
/// `[omitted N text items ...]` sentinel is appended.
pub fn truncate_function_output_items(
    items: &[ContentItem],
    policy: TruncationPolicy,
) -> Vec<ContentItem> {
    let mut out: Vec<ContentItem> = Vec::with_capacity(items.len());
    let mut remaining = policy.byte_budget();
    let mut omitted = 0usize;

    for item in items {
        match item {
            ContentItem::Text(t) => {
                if remaining == 0 {
                    omitted += 1;
                    continue;
                }
                if t.len() <= remaining {
                    out.push(ContentItem::Text(t.clone()));
                    remaining = remaining.saturating_sub(t.len());
                } else {
                    let snippet = truncate_middle_chars(t, remaining);
                    if snippet.is_empty() {
                        omitted += 1;
                    } else {
                        out.push(ContentItem::Text(snippet));
                    }
                    remaining = 0;
                }
            }
            ContentItem::Image { url } => {
                out.push(ContentItem::Image { url: url.clone() });
            }
        }
    }

    if omitted > 0 {
        out.push(ContentItem::Text(format!(
            "[omitted {omitted} text items ...]"
        )));
    }
    out
}

#[cfg(test)]
mod policy_tests {
    use super::*;

    #[test]
    fn byte_budget_returns_inner_value() {
        assert_eq!(TruncationPolicy::Bytes(1024).byte_budget(), 1024);
    }

    #[test]
    fn token_budget_returns_inner_value() {
        assert_eq!(TruncationPolicy::Tokens(500).token_budget(), 500);
    }

    #[test]
    fn token_policy_byte_budget_uses_4x_heuristic() {
        // 1 token ≈ 4 bytes (codex convention)
        assert_eq!(TruncationPolicy::Tokens(100).byte_budget(), 400);
    }

    #[test]
    fn byte_policy_token_budget_divides_by_four() {
        assert_eq!(TruncationPolicy::Bytes(400).token_budget(), 100);
    }
}

#[cfg(test)]
mod middle_chars_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn no_truncation_when_under_budget() {
        assert_eq!(truncate_middle_chars("hello", 10), "hello");
    }

    #[test]
    fn keeps_head_and_tail_with_marker() {
        // Budget 80 is large enough to fit head + tail + marker.
        let input = "0123456789abcdef".repeat(10); // 160 bytes
        let out = truncate_middle_chars(&input, 80);
        assert!(out.contains("[...] omitted "), "missing marker: {out}");
        assert!(out.starts_with('0'), "head preserved");
        assert!(out.ends_with('f'), "tail preserved");
        assert!(out.len() <= 120, "marker overhead bounded");
    }

    #[test]
    fn handles_multibyte_at_boundary() {
        // 'é' is 2 bytes in UTF-8. Budget=4 must not split it.
        let s = "aéaéaéaéaé";
        let out = truncate_middle_chars(s, 6);
        assert!(out.is_char_boundary(out.len()));
    }

    #[test]
    fn empty_input_is_empty() {
        assert_eq!(truncate_middle_chars("", 100), "");
    }

    #[test]
    fn budget_zero_returns_marker_only() {
        let out = truncate_middle_chars("abc", 0);
        assert!(out.contains("[...] omitted "));
    }
}

#[cfg(test)]
mod formatted_tests {
    use super::*;

    #[test]
    fn no_prefix_when_under_budget() {
        let out = formatted_truncate_text("short", TruncationPolicy::Bytes(100));
        assert_eq!(out, "short");
    }

    #[test]
    fn prefixes_with_total_lines_when_truncated() {
        let big = "line\n".repeat(2000); // ~10000 bytes
        let out = formatted_truncate_text(&big, TruncationPolicy::Bytes(200));
        assert!(out.starts_with("Total output lines: 2000\n\n"), "got: {out}");
    }

    #[test]
    fn token_policy_uses_byte_equivalent() {
        let big = "x".repeat(1000);
        let out = formatted_truncate_text(&big, TruncationPolicy::Tokens(50)); // ≈ 200 bytes
        assert!(out.starts_with("Total output lines: 1\n\n"));
        assert!(out.len() <= 300);
    }
}
