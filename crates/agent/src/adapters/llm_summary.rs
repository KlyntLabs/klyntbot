//! LlmSummaryProvider — calls an LLM to produce abstractive conversation summaries.
//!
//! Segments are grouped into sub-batches and summarized in parallel, with each
//! sub-batch producing a JSON array of summaries via a single LLM call.

use std::fmt::Write;

use async_trait::async_trait;
use context_engine::SummaryProvider;
use providers::{ChatParams, Message, UserContent};
use tracing::warn;

/// Maximum conversation segments to include in a single LLM call.
/// Balances output quality against call count.
const MAX_SEGMENTS_PER_CALL: usize = 5;

/// Implements [`SummaryProvider`] by delegating to a configured LLM provider.
pub struct LlmSummaryProvider {
    provider: providers::DynProvider,
    model: String,
}

impl LlmSummaryProvider {
    pub fn new(provider: providers::DynProvider, model: String) -> Self {
        Self { provider, model }
    }

    /// Format a single conversation segment for inclusion in the prompt.
    fn format_segment(messages: &[Message]) -> String {
        messages
            .iter()
            .filter_map(|m| match m {
                Message::User { content } => {
                    let text = match content {
                        UserContent::Text(t) => t.as_str(),
                        UserContent::MultiPart(_) => "[multipart]",
                    };
                    Some(format!("User: {}", text))
                }
                Message::Assistant {
                    content: Some(text),
                    ..
                } => Some(format!("Assistant: {}", text)),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Build a prompt that asks the LLM to summarize multiple segments at once,
    /// returning a JSON array of summary strings.
    fn build_batch_prompt(segments: &[Vec<Message>]) -> String {
        let mut prompt = format!(
            "Summarize each numbered conversation segment in 2-3 sentences, \
             preserving key facts, decisions, and action items.\n\n\
             Return ONLY a JSON array of exactly {} strings, one summary per segment.\n",
            segments.len()
        );
        for (i, segment) in segments.iter().enumerate() {
            let _ = write!(
                prompt,
                "\n=== Segment {} ===\n{}\n",
                i + 1,
                Self::format_segment(segment)
            );
        }
        prompt
    }

    /// Try to extract a `Vec<String>` JSON array from LLM output,
    /// delegating bracket-finding to `common::helpers::extract_json_array`.
    fn extract_json(text: &str) -> Option<Vec<String>> {
        let slice = common::helpers::extract_json_array(text.trim());
        serde_json::from_str::<Vec<String>>(slice).ok()
    }

    /// Execute a single LLM call covering one sub-batch of segments.
    async fn call_batch(&self, segments: &[Vec<Message>]) -> Vec<Result<String, String>> {
        let n = segments.len();
        let prompt = Self::build_batch_prompt(segments);
        let request_messages = vec![Message::user(prompt)];
        let max_tokens = (100 * n).max(256) as u32;
        let params = ChatParams::new(&self.model).with_max_tokens(max_tokens);

        match self.provider.chat(&request_messages, None, &params).await {
            Ok(response) => {
                if let Some(content) = response.content {
                    if let Some(summaries) = Self::extract_json(&content) {
                        if summaries.len() == n {
                            return summaries.into_iter().map(Ok).collect();
                        }
                        warn!(
                            "LlmSummaryProvider: expected {} summaries, got {}",
                            n,
                            summaries.len()
                        );
                    } else {
                        warn!("LlmSummaryProvider: failed to parse JSON from response");
                    }
                }
                vec![Err("Failed to parse batch summary response".into()); n]
            }
            Err(e) => {
                warn!("LlmSummaryProvider batch call failed: {}", e);
                vec![Err(e.to_string()); n]
            }
        }
    }
}

#[async_trait]
impl SummaryProvider for LlmSummaryProvider {
    async fn summarize_batch(&self, segments: Vec<Vec<Message>>) -> Vec<Result<String, String>> {
        if segments.is_empty() {
            return vec![];
        }

        // Split into sub-batches and run in parallel
        let futures = segments
            .chunks(MAX_SEGMENTS_PER_CALL)
            .map(|batch| self.call_batch(batch));
        let results = futures_util::future::join_all(futures).await;
        results.into_iter().flatten().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_json_raw_array() {
        let input = r#"["First summary.", "Second summary."]"#;
        let result = LlmSummaryProvider::extract_json(input).unwrap();
        assert_eq!(result, vec!["First summary.", "Second summary."]);
    }

    #[test]
    fn extract_json_markdown_fenced() {
        let input = "Here are the summaries:\n```json\n[\"One.\", \"Two.\"]\n```\n";
        let result = LlmSummaryProvider::extract_json(input).unwrap();
        assert_eq!(result, vec!["One.", "Two."]);
    }

    #[test]
    fn extract_json_with_surrounding_text() {
        let input = "Sure! [\"Summary A.\", \"Summary B.\"] Hope that helps.";
        let result = LlmSummaryProvider::extract_json(input).unwrap();
        assert_eq!(result, vec!["Summary A.", "Summary B."]);
    }

    #[test]
    fn extract_json_invalid_returns_none() {
        assert!(LlmSummaryProvider::extract_json("no json here").is_none());
        assert!(LlmSummaryProvider::extract_json("").is_none());
    }

    #[test]
    fn build_batch_prompt_contains_all_segments() {
        let segments = vec![
            vec![Message::user("Hello"), Message::assistant("Hi!")],
            vec![Message::user("Bye"), Message::assistant("Goodbye!")],
        ];
        let prompt = LlmSummaryProvider::build_batch_prompt(&segments);
        assert!(prompt.contains("exactly 2 strings"));
        assert!(prompt.contains("=== Segment 1 ==="));
        assert!(prompt.contains("=== Segment 2 ==="));
        assert!(prompt.contains("User: Hello"));
        assert!(prompt.contains("Assistant: Goodbye!"));
    }
}
