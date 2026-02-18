use providers::{Message, UserContent};

/// Summary of a range of compressed messages.
pub struct HistorySummary {
    /// The summarized content text.
    pub content: String,
    /// Indices (start, end) of the original messages this summary covers.
    pub message_range: (usize, usize),
    /// Estimated token count for this summary.
    pub token_count: usize,
}

/// Result of compressing conversation history.
pub struct CompressedHistory {
    /// Summaries of older messages that were compressed.
    pub summaries: Vec<HistorySummary>,
    /// Recent messages kept verbatim.
    pub recent_messages: Vec<Message>,
    /// Estimated total token count across summaries + recent messages.
    pub total_tokens: usize,
}

/// Compresses conversation history to fit within a token budget.
///
/// Strategy:
/// - Always keep at least `min_recent_messages` verbatim (from the end)
/// - Expand recent window if budget allows
/// - Summarize older messages using extractive summarization (no LLM call)
pub struct HistoryCompressor {
    min_recent_messages: usize,
}

impl HistoryCompressor {
    pub fn new(min_recent: usize) -> Self {
        Self {
            min_recent_messages: min_recent,
        }
    }

    pub fn compress(&self, history: &[Message], budget_tokens: usize) -> CompressedHistory {
        if history.is_empty() {
            return CompressedHistory {
                summaries: vec![],
                recent_messages: vec![],
                total_tokens: 0,
            };
        }

        // Always keep at least min_recent messages (or all if fewer)
        let min_keep = self.min_recent_messages.min(history.len());

        // Count tokens for the minimum recent messages
        let mut recent_tokens: usize = history[history.len() - min_keep..]
            .iter()
            .map(Self::estimate_tokens)
            .sum();

        // Try to include more recent messages if budget allows (use half budget for extra)
        let mut extra_count = 0;
        let older_messages = &history[..history.len() - min_keep];
        let half_remaining = budget_tokens.saturating_sub(recent_tokens) / 2;
        let mut extra_tokens = 0;

        for msg in older_messages.iter().rev() {
            let t = Self::estimate_tokens(msg);
            if extra_tokens + t <= half_remaining {
                extra_tokens += t;
                extra_count += 1;
            } else {
                break;
            }
        }

        recent_tokens += extra_tokens;
        let recent_count = min_keep + extra_count;
        let split_point = history.len() - recent_count;

        let to_summarize = &history[..split_point];
        let recent_messages = history[split_point..].to_vec();

        // Summarize older messages in chunks
        let mut summaries = Vec::new();
        if !to_summarize.is_empty() {
            let chunk_size = 5;
            for (chunk_idx, chunk) in to_summarize.chunks(chunk_size).enumerate() {
                let content = Self::extractive_summary(chunk);
                let token_count = content.len() / 4;
                let start = chunk_idx * chunk_size;
                let end = (start + chunk.len()).min(split_point);
                summaries.push(HistorySummary {
                    content,
                    message_range: (start, end),
                    token_count,
                });
            }
        }

        let summary_tokens: usize = summaries.iter().map(|s| s.token_count).sum();
        let total_tokens = recent_tokens + summary_tokens;

        CompressedHistory {
            summaries,
            recent_messages,
            total_tokens,
        }
    }

    /// Extractive summary: takes the first sentence/snippet from each message.
    pub fn extractive_summary(messages: &[Message]) -> String {
        let mut lines = vec!["Earlier in this conversation:".to_string()];
        for msg in messages {
            match msg {
                Message::User { content } => {
                    let text = match content {
                        UserContent::Text(t) => t.clone(),
                        UserContent::MultiPart(_) => "[multipart message]".to_string(),
                    };
                    let snippet = first_snippet(&text, 100);
                    lines.push(format!("- User: {}", snippet));
                }
                Message::Assistant {
                    content: Some(text),
                    ..
                } => {
                    let snippet = first_snippet(text, 100);
                    lines.push(format!("- Assistant: {}", snippet));
                }
                _ => {}
            }
        }
        lines.join("\n")
    }

    fn estimate_tokens(msg: &Message) -> usize {
        match msg {
            Message::System { content } => content.len() / 4,
            Message::User { content } => match content {
                UserContent::Text(t) => t.len() / 4,
                UserContent::MultiPart(parts) => parts.len() * 10,
            },
            Message::Assistant { content, .. } => {
                content.as_deref().map(|t| t.len() / 4).unwrap_or(0) + 20
            }
            Message::Tool { content, .. } => content.len() / 4 + 10,
        }
    }
}

/// Extract the first sentence or up to `max_chars` characters.
fn first_snippet(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= max_chars {
        trimmed.to_string()
    } else {
        let cut = trimmed[..max_chars]
            .rfind(['.', '!', '?', '\n'])
            .unwrap_or(max_chars);
        format!("{}…", &trimmed[..cut])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_history(n: usize) -> Vec<Message> {
        let mut msgs = Vec::new();
        for i in 0..n {
            if i % 2 == 0 {
                msgs.push(Message::user(format!("User message {}", i)));
            } else {
                msgs.push(Message::assistant(format!("Assistant response {}", i)));
            }
        }
        msgs
    }

    #[test]
    fn test_recent_messages_kept_verbatim() {
        let compressor = HistoryCompressor::new(4);
        let history = make_history(20);
        let result = compressor.compress(&history, 50_000);

        assert!(!result.recent_messages.is_empty());
        // Last message in result should match last message in history
        let last_original = &history[history.len() - 1];
        let last_compressed = &result.recent_messages[result.recent_messages.len() - 1];
        if let (
            Message::Assistant {
                content: Some(a), ..
            },
            Message::Assistant {
                content: Some(b), ..
            },
        ) = (last_original, last_compressed)
        {
            assert_eq!(a, b);
        } else {
            panic!("Last messages should match");
        }
    }

    #[test]
    fn test_min_recent_always_enforced() {
        let compressor = HistoryCompressor::new(4);
        let history = make_history(10);
        let result = compressor.compress(&history, 1); // tiny budget
                                                       // Even with tiny budget, we keep min_recent messages
        assert!(result.recent_messages.len() >= 4_usize.min(history.len()));
    }

    #[test]
    fn test_empty_history_returns_empty() {
        let compressor = HistoryCompressor::new(4);
        let result = compressor.compress(&[], 10_000);
        assert!(result.recent_messages.is_empty());
        assert!(result.summaries.is_empty());
        assert_eq!(result.total_tokens, 0);
    }

    #[test]
    fn test_small_history_all_verbatim() {
        let compressor = HistoryCompressor::new(4);
        let history = make_history(3);
        let result = compressor.compress(&history, 50_000);
        assert_eq!(result.recent_messages.len(), 3);
        assert!(result.summaries.is_empty());
    }

    #[test]
    fn test_summary_format_starts_correctly() {
        let msgs = vec![
            Message::user("Hello there"),
            Message::assistant("Hi! How can I help?"),
        ];
        let summary = HistoryCompressor::extractive_summary(&msgs);
        assert!(summary.starts_with("Earlier in this conversation:"));
        assert!(summary.contains("Hello there") || summary.contains("Hi!"));
    }
}
