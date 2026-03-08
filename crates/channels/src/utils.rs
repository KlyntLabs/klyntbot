//! Message splitting utilities for per-channel length limits.

use common::utils::truncate_at_boundary;

/// Per-channel message length limits.
pub fn max_length(channel: &str) -> usize {
    match channel {
        "telegram" => 4096,
        "discord" => 2000,
        "slack" => 8000,
        "email" => 8000,
        _ => 4000, // conservative default
    }
}

/// Split a message into chunks that fit within the given character limit.
///
/// Splitting priority:
/// 1. Paragraph breaks (double newlines)
/// 2. Line breaks (single newlines)
/// 3. Sentence breaks (`. `, `! `, `? `)
/// 4. Word breaks (spaces)
/// 5. Hard character split (last resort)
pub fn split_message(text: &str, limit: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![];
    }
    if text.len() <= limit {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if remaining.len() <= limit {
            chunks.push(remaining.to_string());
            break;
        }

        let split_at = find_split_point(remaining, limit);
        let (chunk, rest) = remaining.split_at(split_at);
        chunks.push(chunk.to_string());
        remaining = rest.trim_start_matches(['\n', ' ']);
    }

    chunks
}

/// Find the best split point within the limit.
fn find_split_point(text: &str, limit: usize) -> usize {
    let search = truncate_at_boundary(text, limit);

    // 1. Try paragraph break (double newline)
    if let Some(pos) = search.rfind("\n\n") {
        if pos > 0 {
            return pos;
        }
    }

    // 2. Try line break
    if let Some(pos) = search.rfind('\n') {
        if pos > 0 {
            return pos;
        }
    }

    // 3. Try sentence break (include punctuation, exclude trailing space)
    for pat in &[". ", "! ", "? "] {
        if let Some(pos) = search.rfind(pat) {
            if pos > 0 {
                return pos + 1; // split after the punctuation character
            }
        }
    }

    // 4. Try word break
    if let Some(pos) = search.rfind(' ') {
        if pos > 0 {
            return pos;
        }
    }

    // 5. Hard split at safe char boundary
    search.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_message_no_split() {
        let chunks = split_message("Hello world", 100);
        assert_eq!(chunks, vec!["Hello world"]);
    }

    #[test]
    fn test_empty_message() {
        let chunks = split_message("", 100);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_multibyte_utf8_no_panic() {
        // CJK characters are 3 bytes each in UTF-8
        let msg = "a]b]c]d]e]f]g]h]i]j]".replace(']', "\u{4e16}"); // 世
        let chunks = split_message(&msg, 10);
        for chunk in &chunks {
            assert!(chunk.len() <= 10);
        }
    }

    #[test]
    fn test_emoji_split_no_panic() {
        let msg = "Hello 🌍🌍🌍🌍🌍 World 🌍🌍🌍🌍🌍";
        let chunks = split_message(msg, 15);
        for chunk in &chunks {
            assert!(chunk.len() <= 15);
        }
    }

    #[test]
    fn test_exact_limit() {
        let msg = "a".repeat(100);
        let chunks = split_message(&msg, 100);
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn test_split_on_paragraph() {
        let msg = format!("{}\n\n{}", "a".repeat(50), "b".repeat(50));
        let chunks = split_message(&msg, 60);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], "a".repeat(50));
        assert_eq!(chunks[1], "b".repeat(50));
    }

    #[test]
    fn test_split_on_newline() {
        let msg = format!("{}\n{}", "a".repeat(50), "b".repeat(50));
        let chunks = split_message(&msg, 60);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], "a".repeat(50));
        assert_eq!(chunks[1], "b".repeat(50));
    }

    #[test]
    fn test_split_on_sentence() {
        let msg = format!("{}. {}", "a".repeat(49), "b".repeat(50));
        let chunks = split_message(&msg, 60);
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].ends_with('.'));
    }

    #[test]
    fn test_split_on_word() {
        let msg = format!("{} {}", "a".repeat(30), "b".repeat(30));
        let chunks = split_message(&msg, 40);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], "a".repeat(30));
    }

    #[test]
    fn test_hard_split() {
        let msg = "a".repeat(200);
        let chunks = split_message(&msg, 100);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), 100);
        assert_eq!(chunks[1].len(), 100);
    }

    #[test]
    fn test_all_chunks_within_limit() {
        let msg = "Hello world. This is a test. Multiple sentences here. And some more text to fill it up.";
        let chunks = split_message(msg, 30);
        for chunk in &chunks {
            assert!(chunk.len() <= 30, "Chunk too long: {} chars", chunk.len());
        }
    }

    #[test]
    fn test_per_channel_limits() {
        assert_eq!(max_length("telegram"), 4096);
        assert_eq!(max_length("discord"), 2000);
        assert_eq!(max_length("slack"), 8000);
        assert_eq!(max_length("email"), 8000);
        assert_eq!(max_length("unknown"), 4000);
    }

    #[test]
    fn test_very_long_message() {
        let msg = "word ".repeat(2000); // ~10000 chars
        let chunks = split_message(&msg, 2000);
        assert!(chunks.len() > 1);
        for chunk in &chunks {
            assert!(chunk.len() <= 2000);
        }
        // Verify no content lost
        let rejoined: String = chunks.join(" ");
        // Allow for trimming differences
        assert!(rejoined.len() >= msg.trim().len() - chunks.len());
    }
}
