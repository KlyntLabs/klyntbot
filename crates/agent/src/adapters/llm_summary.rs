//! LlmSummaryProvider — calls an LLM to produce abstractive conversation summaries.
//!
//! Segments are grouped into sub-batches and summarized in parallel, with each
//! sub-batch producing a JSON array of summaries via a single LLM call.

use std::fmt::Write;

use async_trait::async_trait;
use context_engine::{CompressionTier, SummaryProvider, TIER1_INSTRUCTIONS, TIER2_INSTRUCTIONS};
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
    fn build_batch_prompt(segments: &[Vec<Message>], tier: CompressionTier) -> String {
        let instructions = match tier {
            CompressionTier::Detailed => TIER1_INSTRUCTIONS,
            CompressionTier::Condensed => TIER2_INSTRUCTIONS,
        };

        let mut prompt = format!(
            "{}\n\nReturn ONLY a JSON array of exactly {} strings, one summary per turn.\n\
             No extra text.\n",
            instructions,
            segments.len()
        );
        for (i, segment) in segments.iter().enumerate() {
            let _ = write!(
                prompt,
                "\n=== Turn {} ===\n{}\n",
                i + 1,
                Self::format_segment(segment)
            );
        }
        prompt
    }

    /// Try to extract a `Vec<String>` JSON array from LLM output,
    /// delegating bracket-finding to `common::helpers::extract_json_array`.
    fn extract_json(text: &str) -> Option<Vec<String>> {
        // Reasoning models (Mimo, DeepSeek-R1) emit chain-of-thought prose
        // around the final answer; the prose itself can contain stray
        // brackets, so the naive first-`[` to last-`]` slice spans
        // unrelated content. Prefer the LAST balanced top-level array;
        // fall back to the legacy slice when none is found.
        let stripped = common::helpers::strip_llm_fences(text);
        if let Some(slice) = common::helpers::extract_last_json_array(stripped) {
            if let Ok(v) = serde_json::from_str::<Vec<String>>(slice) {
                return Some(v);
            }
        }
        let slice = common::helpers::extract_json_array(stripped.trim());
        serde_json::from_str::<Vec<String>>(slice).ok()
    }

    /// Execute a single LLM call covering one sub-batch of segments.
    async fn call_batch(
        &self,
        segments: &[Vec<Message>],
        tier: CompressionTier,
    ) -> Vec<Result<String, String>> {
        let n = segments.len();
        let prompt = Self::build_batch_prompt(segments, tier);
        let request_messages = vec![Message::user(prompt)];
        let max_tokens = match tier {
            CompressionTier::Detailed => (150 * n).max(256) as u32,
            CompressionTier::Condensed => (60 * n).max(128) as u32,
        };
        let params = ChatParams::new(&self.model).with_max_tokens(max_tokens);

        match self
            .provider
            .chat(&request_messages, None, &params, &[])
            .await
        {
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
    async fn summarize_batch(
        &self,
        segments: Vec<Vec<Message>>,
        tier: CompressionTier,
    ) -> Vec<Result<String, String>> {
        if segments.is_empty() {
            return vec![];
        }

        // Split into sub-batches and run in parallel
        let futures = segments
            .chunks(MAX_SEGMENTS_PER_CALL)
            .map(|batch| self.call_batch(batch, tier));
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
        let prompt = LlmSummaryProvider::build_batch_prompt(&segments, CompressionTier::Detailed);
        assert!(prompt.contains("exactly 2 strings"));
        assert!(prompt.contains("=== Turn 1 ==="));
        assert!(prompt.contains("=== Turn 2 ==="));
        assert!(prompt.contains("User: Hello"));
        assert!(prompt.contains("Decisions made")); // Tier 1 specific
    }

    #[test]
    fn build_batch_prompt_tier2_uses_condensed_instructions() {
        let segments = vec![vec![Message::user("Test"), Message::assistant("Reply")]];
        let prompt = LlmSummaryProvider::build_batch_prompt(&segments, CompressionTier::Condensed);
        assert!(prompt.contains("ONLY"));
        assert!(prompt.contains("UNRESOLVED"));
        assert!(!prompt.contains("Decisions made"));
    }
}
