//! Mid-loop context compressor for the ReactiveEngine.
//!
//! During long ReAct loops, tool results accumulate and can exhaust the
//! context window. This compressor checks token usage after each iteration
//! and replaces older tool results with extractive summaries when the
//! accumulated tokens exceed a threshold.

use std::sync::Arc;

use context_engine::TokenCounter;
use providers::Message;
use tracing::info;

/// Compress when accumulated tokens exceed this fraction of context_window.
const COMPRESSION_THRESHOLD: f64 = 0.70;

/// Number of recent messages to always keep verbatim (from the end of the vec).
const MIN_RECENT_MESSAGES: usize = 8;

/// Maximum length of a compressed tool result summary (chars).
const SUMMARY_SNIPPET_LENGTH: usize = 150;

/// Skip compressing tool results shorter than this many tokens.
const MIN_COMPRESSIBLE_TOKENS: usize = 50;

pub struct MidLoopCompressor {
    token_counter: Arc<dyn TokenCounter>,
    threshold: usize,
}

impl MidLoopCompressor {
    pub fn new(token_counter: Arc<dyn TokenCounter>, context_window: usize) -> Self {
        Self {
            token_counter,
            threshold: (context_window as f64 * COMPRESSION_THRESHOLD) as usize,
        }
    }

    pub fn estimate_tokens(&self, messages: &[Message]) -> usize {
        messages
            .iter()
            .map(|m| context_engine::estimate_message_tokens(&*self.token_counter, m))
            .sum()
    }

    /// Compress older tool results if total tokens exceed the threshold.
    ///
    /// Strategy: count tokens, skip if under threshold, then replace older
    /// Tool messages (outside the recent window) with extractive summaries.
    /// Returns `Some((before_tokens, after_tokens, messages_compacted))` if applied, `None` otherwise.
    pub fn compress_if_needed(&self, messages: &mut [Message]) -> Option<(usize, usize, usize)> {
        // Early exit: not enough messages to have anything outside the recent window
        if messages.len() <= MIN_RECENT_MESSAGES + 1 {
            return None;
        }

        let total_tokens = self.estimate_tokens(messages);

        if total_tokens <= self.threshold {
            return None;
        }

        info!(
            total_tokens,
            self.threshold,
            message_count = messages.len(),
            "mid-loop compression triggered"
        );

        let system_count = messages
            .iter()
            .take_while(|m| matches!(m, Message::System { .. }))
            .count();

        let recent_start = messages
            .len()
            .saturating_sub(MIN_RECENT_MESSAGES)
            .max(system_count);

        // Accumulate savings inline to avoid a second full scan
        let mut saved_tokens: usize = 0;
        let mut messages_compacted: usize = 0;
        for msg in messages[system_count..recent_start].iter_mut() {
            if let Message::Tool { content, name, .. } = msg {
                let original_text = content.as_text();
                let image_count = content.image_part_count();
                let original_tokens =
                    self.token_counter.estimate_text(&original_text) + image_count * 1024;
                if original_tokens > MIN_COMPRESSIBLE_TOKENS {
                    messages_compacted += 1;
                    let summary_text = format!(
                        "{}... [compressed {name} result, originally {} chars + {} image part(s)]",
                        context_engine::first_snippet(&original_text, SUMMARY_SNIPPET_LENGTH),
                        original_text.len(),
                        image_count,
                    );
                    let new_tokens = self.token_counter.estimate_text(&summary_text);
                    saved_tokens += original_tokens.saturating_sub(new_tokens);
                    // Image parts are dropped; text becomes the summary. Compression
                    // is one-way; the original payload is gone after this step.
                    *content = providers::ToolContent::Text(summary_text);
                }
            }
        }

        let new_tokens = total_tokens.saturating_sub(saved_tokens);
        info!(
            before = total_tokens,
            after = new_tokens,
            saved = saved_tokens,
            "mid-loop compression complete"
        );

        Some((total_tokens, new_tokens, messages_compacted))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_compressor(context_window: usize) -> MidLoopCompressor {
        MidLoopCompressor::new(Arc::new(context_engine::CharTokenCounter), context_window)
    }

    fn system_msg(text: &str) -> Message {
        Message::System {
            content: text.to_string(),
        }
    }

    fn user_msg(text: &str) -> Message {
        Message::user(text)
    }

    fn assistant_msg(text: &str) -> Message {
        Message::Assistant {
            content: Some(text.to_string()),
            tool_calls: None,
            reasoning_content: None,
        }
    }

    fn tool_msg(id: &str, name: &str, result: &str) -> Message {
        Message::Tool {
            tool_call_id: id.to_string(),
            name: name.to_string(),
            content: providers::ToolContent::Text(result.to_string()),
        }
    }

    #[test]
    fn no_compression_under_threshold() {
        let compressor = make_compressor(10_000);
        let mut messages = vec![
            system_msg("System prompt"),
            user_msg("Hello"),
            assistant_msg("I'll help"),
            tool_msg("1", "tasks", "result 1"),
        ];
        let original_len = messages.len();
        let result = compressor.compress_if_needed(&mut messages);
        assert!(result.is_none(), "should not compress under threshold");
        assert_eq!(messages.len(), original_len);
    }

    #[test]
    fn compression_over_threshold() {
        // Context window of 200 tokens (~800 chars). Fill with large tool results.
        // Need >8 messages so the "recent window" doesn't cover everything.
        let compressor = make_compressor(200);
        let large_content = "x".repeat(400);
        let mut messages = vec![
            system_msg("System prompt"),
            // Iteration 1 (old — will be compressed)
            user_msg("Do something"),
            assistant_msg("Calling tool"),
            tool_msg("1", "web_fetch", &large_content),
            // Iteration 2 (old — will be compressed)
            user_msg("Continue"),
            assistant_msg("Calling another tool"),
            tool_msg("2", "web_fetch", &"y".repeat(400)),
            // Iteration 3 (recent — protected by MIN_RECENT_MESSAGES)
            user_msg("More"),
            assistant_msg("One more tool"),
            tool_msg("3", "web_fetch", &"z".repeat(400)),
            // Iteration 4 (recent)
            user_msg("Final"),
            assistant_msg("Done"),
        ];
        let result = compressor.compress_if_needed(&mut messages);
        assert!(result.is_some(), "should have triggered compression");
        let (before, after, _compacted) = result.unwrap();
        assert!(
            after < before,
            "after ({after}) should be less than before ({before})"
        );
        assert!(matches!(&messages[0], Message::System { .. }));
        if let Message::Tool { content, .. } = &messages[3] {
            assert!(
                content.as_text().contains("[compressed"),
                "older tool should contain compression marker"
            );
            assert!(
                content.as_text().len() < large_content.len(),
                "compressed content should be shorter"
            );
        } else {
            panic!("expected Tool message at index 3");
        }
    }

    #[test]
    fn system_messages_never_compressed() {
        let compressor = make_compressor(50);
        let mut messages = vec![
            system_msg("Important system prompt that must survive"),
            user_msg("query"),
            assistant_msg("calling tool"),
            tool_msg("1", "big_tool", &"z".repeat(500)),
            // Pad to exceed MIN_RECENT_MESSAGES + 1
            user_msg("q2"),
            assistant_msg("a2"),
            tool_msg("2", "t2", &"z".repeat(500)),
            user_msg("q3"),
            assistant_msg("a3"),
            tool_msg("3", "t3", "short"),
        ];
        let _ = compressor.compress_if_needed(&mut messages);
        assert!(
            matches!(&messages[0], Message::System { content } if content.contains("Important"))
        );
    }

    #[test]
    fn preserves_recent_window() {
        let compressor = make_compressor(80);
        let mut messages = vec![
            system_msg("sys"),
            // iteration 1 (old)
            user_msg("q1"),
            assistant_msg("a1"),
            tool_msg("1", "t1", &"old".repeat(100)),
            // iteration 2 (old)
            user_msg("q2"),
            assistant_msg("a2"),
            tool_msg("2", "t2", &"old".repeat(100)),
            // iteration 3 (recent — should survive)
            user_msg("q3"),
            assistant_msg("a3"),
            tool_msg("3", "t3", "recent result"),
        ];
        let _ = compressor.compress_if_needed(&mut messages);
        assert!(messages.iter().any(
            |m| matches!(m, Message::Tool { content, .. } if content.as_text() == "recent result")
        ));
    }

    #[test]
    fn early_exit_on_small_message_count() {
        // Even with a tiny context window, too few messages should skip compression
        let compressor = make_compressor(1);
        let mut messages = vec![
            system_msg("sys"),
            user_msg("q"),
            assistant_msg("a"),
            tool_msg("1", "t", &"x".repeat(1000)),
        ];
        let result = compressor.compress_if_needed(&mut messages);
        assert!(
            result.is_none(),
            "should skip when message count is too small"
        );
    }
}
