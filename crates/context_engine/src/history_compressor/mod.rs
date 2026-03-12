mod snippet;
mod types;

pub use types::{CompressedHistory, CompressorConfig, CompressorMode, HistorySummary};

use std::sync::Arc;

use providers::{Message, UserContent};

use crate::summary_provider::SummaryProvider;
use crate::token_counter::{default_token_counter, TokenCounter};

use snippet::first_snippet;
use types::DEFAULT_SNIPPET_LENGTH;

/// Compresses conversation history to fit within a token budget.
///
/// Strategy:
/// - Always keep at least `config.min_recent_messages` verbatim (from the end)
/// - Expand recent window if budget allows
/// - Summarize older messages using extractive summarization (no LLM call)
///   or abstractive summarization via a `SummaryProvider` when configured.
pub struct HistoryCompressor {
    token_counter: Arc<dyn TokenCounter>,
    config: CompressorConfig,
    summary_provider: Option<Arc<dyn SummaryProvider>>,
}

impl HistoryCompressor {
    pub fn new(min_recent: usize, token_counter: Arc<dyn TokenCounter>) -> Self {
        Self {
            token_counter,
            config: CompressorConfig {
                min_recent_messages: min_recent,
                ..CompressorConfig::default()
            },
            summary_provider: None,
        }
    }

    /// Create with a custom configuration.
    pub fn with_config(
        min_recent: usize,
        token_counter: Arc<dyn TokenCounter>,
        config: CompressorConfig,
    ) -> Self {
        Self {
            token_counter,
            config: CompressorConfig {
                min_recent_messages: min_recent,
                ..config
            },
            summary_provider: None,
        }
    }

    /// Create from a `CompressorConfig` directly (uses config's `min_recent_messages`).
    pub fn from_config(token_counter: Arc<dyn TokenCounter>, config: CompressorConfig) -> Self {
        Self {
            token_counter,
            config,
            summary_provider: None,
        }
    }

    /// Create with the default character-based token counter.
    pub fn with_defaults(min_recent: usize) -> Self {
        Self::new(min_recent, default_token_counter())
    }

    /// Set an optional `SummaryProvider` for abstractive compression.
    pub fn with_summary_provider(mut self, provider: Arc<dyn SummaryProvider>) -> Self {
        self.summary_provider = Some(provider);
        self
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
        let min_keep = self.config.min_recent_messages.min(history.len());

        // Count tokens for the minimum recent messages
        let mut recent_tokens: usize = history[history.len() - min_keep..]
            .iter()
            .map(|m| self.estimate_message_tokens(m))
            .sum();

        // Try to include more recent messages if budget allows (use half budget for extra)
        let mut extra_count = 0;
        let older_messages = &history[..history.len() - min_keep];
        let half_remaining = budget_tokens.saturating_sub(recent_tokens) / 2;
        let mut extra_tokens = 0;

        for msg in older_messages.iter().rev() {
            let t = self.estimate_message_tokens(msg);
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
            let chunk_size = self.config.chunk_size;
            let snippet_len = self.config.snippet_length;
            for (chunk_idx, chunk) in to_summarize.chunks(chunk_size).enumerate() {
                let content = Self::extractive_summary_with_length(chunk, snippet_len);
                let token_count = self.token_counter.estimate_text(&content);
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

    /// Async version of [`compress`] that supports abstractive summarization.
    ///
    /// When `mode == Abstractive` and a `SummaryProvider` is configured, each
    /// chunk of older messages is sent to the provider for an LLM-generated
    /// summary. Falls back to extractive summarization on provider error.
    pub async fn compress_async(
        &self,
        history: &[Message],
        budget_tokens: usize,
    ) -> CompressedHistory {
        if history.is_empty() {
            return CompressedHistory {
                summaries: vec![],
                recent_messages: vec![],
                total_tokens: 0,
            };
        }

        // Early-exit: if not abstractive or no provider, delegate to sync compress()
        if self.config.mode != CompressorMode::Abstractive || self.summary_provider.is_none() {
            return self.compress(history, budget_tokens);
        }

        // Same budget/split logic as compress()
        let min_keep = self.config.min_recent_messages.min(history.len());
        let mut recent_tokens: usize = history[history.len() - min_keep..]
            .iter()
            .map(|m| self.estimate_message_tokens(m))
            .sum();
        let mut extra_count = 0;
        let older_messages = &history[..history.len() - min_keep];
        let half_remaining = budget_tokens.saturating_sub(recent_tokens) / 2;
        let mut extra_tokens = 0;
        for msg in older_messages.iter().rev() {
            let t = self.estimate_message_tokens(msg);
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

        let provider = self.summary_provider.as_ref().unwrap();
        let chunk_size = self.config.chunk_size;
        let snippet_len = self.config.snippet_length;

        // Collect chunks with their original ranges, then batch-summarize
        let (segments, ranges): (Vec<Vec<Message>>, Vec<(usize, usize)>) = to_summarize
            .chunks(chunk_size)
            .enumerate()
            .map(|(i, chunk)| {
                let start = i * chunk_size;
                let end = (start + chunk.len()).min(split_point);
                (chunk.to_vec(), (start, end))
            })
            .unzip();

        let results = provider.summarize_batch(segments).await;

        let mut summaries = Vec::with_capacity(results.len());
        for (chunk_idx, result) in results.into_iter().enumerate() {
            let content = match result {
                Ok(text) if !text.is_empty() => text,
                _ => {
                    let (start, end) = ranges[chunk_idx];
                    Self::extractive_summary_with_length(&history[start..end], snippet_len)
                }
            };
            let token_count = self.token_counter.estimate_text(&content);
            summaries.push(HistorySummary {
                content,
                message_range: ranges[chunk_idx],
                token_count,
            });
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
    /// Uses the default snippet length (200 chars).
    pub fn extractive_summary(messages: &[Message]) -> String {
        Self::extractive_summary_with_length(messages, DEFAULT_SNIPPET_LENGTH)
    }

    /// Extractive summary with a configurable snippet length.
    pub fn extractive_summary_with_length(messages: &[Message], snippet_length: usize) -> String {
        let mut lines = vec!["Earlier in this conversation:".to_string()];
        for msg in messages {
            match msg {
                Message::User { content } => {
                    let text = match content {
                        UserContent::Text(t) => t.clone(),
                        UserContent::MultiPart(_) => "[multipart message]".to_string(),
                    };
                    let snip = first_snippet(&text, snippet_length);
                    lines.push(format!("- User: {}", snip));
                }
                Message::Assistant {
                    content: Some(text),
                    ..
                } => {
                    let snip = first_snippet(text, snippet_length);
                    lines.push(format!("- Assistant: {}", snip));
                }
                _ => {}
            }
        }
        lines.join("\n")
    }

    fn estimate_message_tokens(&self, msg: &Message) -> usize {
        match msg {
            Message::System { content } => self.token_counter.estimate_text(content),
            Message::User { content } => match content {
                UserContent::Text(t) => self.token_counter.estimate_text(t),
                UserContent::MultiPart(parts) => parts.len() * 10,
            },
            Message::Assistant { content, .. } => {
                content
                    .as_deref()
                    .map(|t| self.token_counter.estimate_text(t))
                    .unwrap_or(0)
                    + 20
            }
            Message::Tool { content, .. } => self.token_counter.estimate_text(content) + 10,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::summary_provider::SummaryProvider;
    use crate::token_counter::default_token_counter;
    use async_trait::async_trait;

    fn make_compressor() -> HistoryCompressor {
        HistoryCompressor::new(4, default_token_counter())
    }

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
        let compressor = make_compressor();
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
        let compressor = make_compressor();
        let history = make_history(10);
        let result = compressor.compress(&history, 1); // tiny budget
                                                       // Even with tiny budget, we keep min_recent messages
        assert!(result.recent_messages.len() >= 4_usize.min(history.len()));
    }

    #[test]
    fn test_empty_history_returns_empty() {
        let compressor = make_compressor();
        let result = compressor.compress(&[], 10_000);
        assert!(result.recent_messages.is_empty());
        assert!(result.summaries.is_empty());
        assert_eq!(result.total_tokens, 0);
    }

    #[test]
    fn test_small_history_all_verbatim() {
        let compressor = make_compressor();
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

    #[test]
    fn test_custom_token_counter_used() {
        use crate::token_counter::TokenCounter;

        // A counter that always returns 1 per text unit
        struct OnePerCall;
        impl TokenCounter for OnePerCall {
            fn estimate_text(&self, _text: &str) -> usize {
                1
            }
        }

        let compressor = HistoryCompressor::new(2, Arc::new(OnePerCall));
        let history = make_history(6);
        let result = compressor.compress(&history, 100);
        // All summaries should have token_count == 1 (our custom counter returns 1)
        for summary in &result.summaries {
            assert_eq!(summary.token_count, 1);
        }
    }

    // --- G-44: CompressorConfig and CompressorMode tests ---

    #[test]
    fn test_compressor_config_default() {
        let config = CompressorConfig::default();
        assert_eq!(config.snippet_length, 200);
        assert_eq!(config.mode, CompressorMode::Extractive);
        assert_eq!(config.chunk_size, 5);
        assert_eq!(config.min_recent_messages, 4);
    }

    #[test]
    fn test_compressor_mode_abstractive_falls_back() {
        // Abstractive mode currently falls back to Extractive behavior
        let config = CompressorConfig {
            snippet_length: 200,
            mode: CompressorMode::Abstractive,
            ..Default::default()
        };
        let compressor = HistoryCompressor::with_config(4, default_token_counter(), config);
        let history = make_history(10);
        let result = compressor.compress(&history, 50_000);
        // Should still produce results (falls back to extractive)
        assert!(!result.recent_messages.is_empty());
    }

    #[test]
    fn test_custom_snippet_length_in_config() {
        let config = CompressorConfig {
            snippet_length: 50,
            mode: CompressorMode::Extractive,
            ..Default::default()
        };
        let compressor = HistoryCompressor::with_config(2, default_token_counter(), config);

        // Use the compressor to verify it was created with the custom config
        let history = make_history(10);
        let result = compressor.compress(&history, 50_000);
        assert!(!result.recent_messages.is_empty());

        // Also verify the static method with custom length
        let long_text = "A".repeat(300);
        let msgs = vec![Message::user(long_text)];
        let summary = HistoryCompressor::extractive_summary_with_length(&msgs, 50);
        // The user snippet should be truncated
        assert!(summary.len() < 300);
    }

    // --- G-44: Sentence boundary tests ---

    #[test]
    fn test_snippet_cuts_at_sentence_boundary() {
        let text = "First sentence here. Second sentence here. Third sentence that goes on and on and on to fill up more characters beyond the limit.";
        let snippet = first_snippet(text, 60);
        // Should cut at a sentence boundary
        assert!(
            snippet.ends_with("here.…") || snippet.ends_with("here.…"),
            "Expected sentence boundary cut, got: {}",
            snippet
        );
    }

    #[test]
    fn test_snippet_prefers_later_sentence_boundary() {
        let text = "Short. A longer second sentence that fits. Third sentence is also here but extends well past the allowed character limit we set.";
        let snippet = first_snippet(text, 80);
        // Should include as much as possible up to the last sentence boundary within 80 chars
        assert!(snippet.contains("second sentence"), "Got: {}", snippet);
    }

    #[test]
    fn test_snippet_falls_back_to_word_boundary() {
        // No sentence-ending punctuation within limit
        let text = "This is a long message without any sentence-ending punctuation within the first part of the text that just keeps going and going and going";
        let snippet = first_snippet(text, 60);
        // Should cut at a word boundary
        assert!(
            !snippet.contains("  "),
            "Should not have trailing spaces: {}",
            snippet
        );
        assert!(
            snippet.ends_with('…'),
            "Should end with ellipsis: {}",
            snippet
        );
    }

    #[test]
    fn test_snippet_short_text_unchanged() {
        let text = "Hello!";
        let snippet = first_snippet(text, 200);
        assert_eq!(snippet, "Hello!");
    }

    #[test]
    fn test_snippet_exact_limit_unchanged() {
        let text = "X".repeat(200);
        let snippet = first_snippet(&text, 200);
        assert_eq!(snippet, text);
    }

    #[test]
    fn test_snippet_respects_newline_boundary() {
        let text = "First line\nSecond line that is much much longer and goes way past the character limit we have set for this test";
        let snippet = first_snippet(text, 40);
        assert!(
            snippet.contains("First line") || snippet.contains("Second"),
            "Got: {}",
            snippet
        );
    }

    #[test]
    fn test_snippet_does_not_cut_mid_abbreviation() {
        // "Dr.Smith" should not be treated as sentence end because no space follows
        let text = "Talk to Dr.Smith about the project details and scheduling requirements that go on for a while.";
        let snippet = first_snippet(text, 50);
        // Should NOT cut at "Dr." since no space follows
        assert!(
            !snippet.ends_with("Dr.…"),
            "Should not cut at abbreviation: {}",
            snippet
        );
    }

    // ── Abstractive compression tests ────────────────────────────────────

    struct MockSummaryProvider {
        fixed_response: String,
    }

    impl MockSummaryProvider {
        fn new(response: &str) -> Self {
            Self {
                fixed_response: response.to_string(),
            }
        }
    }

    #[async_trait]
    impl SummaryProvider for MockSummaryProvider {
        async fn summarize_batch(
            &self,
            segments: Vec<Vec<Message>>,
        ) -> Vec<Result<String, String>> {
            segments
                .iter()
                .map(|_| Ok(self.fixed_response.clone()))
                .collect()
        }
    }

    #[tokio::test]
    async fn test_abstractive_mode_calls_summary_provider() {
        let provider = Arc::new(MockSummaryProvider::new("LLM summary of conversation"));
        let config = CompressorConfig {
            mode: CompressorMode::Abstractive,
            min_recent_messages: 4,
            chunk_size: 5,
            ..Default::default()
        };
        let compressor = HistoryCompressor::from_config(default_token_counter(), config)
            .with_summary_provider(provider);
        let history = make_history(20);
        // Use a small budget (30 tokens) to force older messages into summaries.
        // With ~3 tokens/message and 4 min_recent, ~12 tokens used by recent;
        // half_remaining ≈ 9 → only ~3 extra messages fit, leaving ~13 for summarization.
        let result = compressor.compress_async(&history, 30).await;
        assert!(
            !result.summaries.is_empty(),
            "Expected non-empty summaries with tight budget"
        );
        assert!(
            result
                .summaries
                .iter()
                .any(|s| s.content.contains("LLM summary")),
            "Expected at least one summary containing 'LLM summary', got: {:?}",
            result
                .summaries
                .iter()
                .map(|s| &s.content)
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn test_abstractive_fallback_when_no_provider() {
        let config = CompressorConfig {
            mode: CompressorMode::Abstractive,
            ..Default::default()
        };
        let compressor = HistoryCompressor::from_config(default_token_counter(), config);
        let history = make_history(10);
        let result = compressor.compress_async(&history, 50_000).await;
        // Falls back to sync compress() — should still have results
        assert!(!result.recent_messages.is_empty());
    }

    struct FailingSummaryProvider;

    #[async_trait]
    impl SummaryProvider for FailingSummaryProvider {
        async fn summarize_batch(
            &self,
            segments: Vec<Vec<Message>>,
        ) -> Vec<Result<String, String>> {
            segments
                .iter()
                .map(|_| Err("Provider unavailable".into()))
                .collect()
        }
    }

    #[tokio::test]
    async fn test_abstractive_fallback_on_provider_error() {
        let config = CompressorConfig {
            mode: CompressorMode::Abstractive,
            ..Default::default()
        };
        let compressor = HistoryCompressor::from_config(default_token_counter(), config)
            .with_summary_provider(Arc::new(FailingSummaryProvider));
        let history = make_history(20);
        let result = compressor.compress_async(&history, 50_000).await;
        // Should still succeed, falling back to extractive per chunk
        assert!(!result.summaries.is_empty() || !result.recent_messages.is_empty());
        for summary in &result.summaries {
            assert!(
                summary.content.contains("Earlier in this conversation:"),
                "Expected extractive fallback, got: {}",
                summary.content
            );
        }
    }
}
