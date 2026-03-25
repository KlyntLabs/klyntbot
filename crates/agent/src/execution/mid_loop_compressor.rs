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

/// Threshold: compress when accumulated tokens exceed this fraction of context_window.
const COMPRESSION_THRESHOLD: f64 = 0.70;

/// Number of recent messages to always keep verbatim (from the end of the vec).
const MIN_RECENT_MESSAGES: usize = 8;

/// Maximum length of a compressed tool result summary (chars).
const SUMMARY_SNIPPET_LENGTH: usize = 150;

pub struct MidLoopCompressor {
    token_counter: Arc<dyn TokenCounter>,
    context_window: usize,
}

impl MidLoopCompressor {
    pub fn new(token_counter: Arc<dyn TokenCounter>, context_window: usize) -> Self {
        Self {
            token_counter,
            context_window,
        }
    }

    /// Estimate the total token count of the message vec.
    fn estimate_tokens(&self, messages: &[Message]) -> usize {
        messages
            .iter()
            .map(|m| self.estimate_message_tokens(m))
            .sum()
    }

    fn estimate_message_tokens(&self, msg: &Message) -> usize {
        match msg {
            Message::System { content } => self.token_counter.estimate_text(content) + 4,
            Message::User { content } => {
                let text = match content {
                    providers::UserContent::Text(t) => t.as_str(),
                    providers::UserContent::MultiPart(parts) => {
                        return parts.len() * 10; // flat heuristic for multipart
                    }
                };
                self.token_counter.estimate_text(text) + 4
            }
            Message::Assistant { content, .. } => {
                content
                    .as_deref()
                    .map(|c| self.token_counter.estimate_text(c))
                    .unwrap_or(0)
                    + 20 // overhead for tool_calls JSON
            }
            Message::Tool { content, name, .. } => {
                self.token_counter.estimate_text(content)
                    + self.token_counter.estimate_text(name)
                    + 10
            }
        }
    }

    /// Compress older tool results if total tokens exceed the threshold.
    ///
    /// Strategy:
    /// 1. Count total tokens across all messages
    /// 2. If under threshold, return without changes
    /// 3. Split messages into: system prefix + older body + recent tail
    /// 4. Replace Tool messages in the older body with truncated summaries
    ///
    /// Returns `Some((before_tokens, after_tokens))` if compression was applied, `None` otherwise.
    pub fn compress_if_needed(&self, messages: &mut Vec<Message>) -> Option<(usize, usize)> {
        let total_tokens = self.estimate_tokens(messages);
        let threshold = (self.context_window as f64 * COMPRESSION_THRESHOLD) as usize;

        if total_tokens <= threshold {
            return None;
        }

        info!(
            total_tokens,
            threshold,
            message_count = messages.len(),
            "mid-loop compression triggered"
        );

        // Preserve system messages at the front
        let system_count = messages
            .iter()
            .take_while(|m| matches!(m, Message::System { .. }))
            .count();

        // Preserve recent tail verbatim
        let recent_start = messages
            .len()
            .saturating_sub(MIN_RECENT_MESSAGES)
            .max(system_count);

        // Compress tool results in the older body (between system prefix and recent tail)
        for msg in messages[system_count..recent_start].iter_mut() {
            if let Message::Tool { content, name, .. } = msg {
                let original_tokens = self.token_counter.estimate_text(content);
                if original_tokens > 50 {
                    *content = Self::summarize_tool_result(name, content);
                }
            }
        }

        let new_tokens = self.estimate_tokens(messages);
        info!(
            before = total_tokens,
            after = new_tokens,
            saved = total_tokens.saturating_sub(new_tokens),
            "mid-loop compression complete"
        );

        Some((total_tokens, new_tokens))
    }

    /// Create a short summary of a tool result.
    fn summarize_tool_result(tool_name: &str, content: &str) -> String {
        let trimmed = content.trim();
        if trimmed.len() <= SUMMARY_SNIPPET_LENGTH {
            return trimmed.to_string();
        }
        // Take first SUMMARY_SNIPPET_LENGTH chars, find a clean break point
        let snippet: String = trimmed.chars().take(SUMMARY_SNIPPET_LENGTH).collect();
        let break_point = snippet
            .rfind('\n')
            .or_else(|| snippet.rfind(". "))
            .or_else(|| snippet.rfind(' '))
            .unwrap_or(snippet.len());
        format!(
            "{}... [compressed {tool_name} result, originally {} chars]",
            &snippet[..break_point],
            trimmed.len()
        )
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
            content: result.to_string(),
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
        let (before, after) = result.unwrap();
        assert!(after < before, "after ({after}) should be less than before ({before})");
        // System prompt should survive
        assert!(matches!(&messages[0], Message::System { .. }));
        // The older tool result (index 3) should be compressed — content shortened
        if let Message::Tool { content, .. } = &messages[3] {
            assert!(
                content.contains("[compressed"),
                "older tool should contain compression marker"
            );
            assert!(
                content.len() < large_content.len(),
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
        // Most recent tool result should be preserved verbatim
        assert!(messages
            .iter()
            .any(|m| matches!(m, Message::Tool { content, .. } if content == "recent result")));
    }
}
