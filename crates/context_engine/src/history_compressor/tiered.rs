use std::sync::Arc;

use providers::Message;

use crate::memory_scorer::MemoryScorer;
use crate::summary_provider::SummaryProvider;
use crate::token_counter::{self, TokenCounter};
use config::schema::HistoryCompressionConfig;

use super::grouping::group_into_turns;
use super::snippet::first_snippet;
use super::types::{
    AssignedTier, CompressedHistory, CompressionTier, ConversationTurn, TierSummary,
    DEFAULT_SNIPPET_LENGTH,
};

/// Tiered history compressor.
///
/// Replaces the old `HistoryCompressor`. Groups messages into conversation
/// turns, optionally scores them via cognitive relevance, assigns tiers
/// (Verbatim / Detailed / Condensed), and compresses with tier-specific
/// LLM prompts. Falls back to extractive snippets when LLM calls fail
/// or when extractive already fits the budget.
pub struct TieredHistoryCompressor {
    token_counter: Arc<dyn TokenCounter>,
    summary_provider: Option<Arc<dyn SummaryProvider>>,
    memory_scorer: Option<Arc<dyn MemoryScorer>>,
    config: HistoryCompressionConfig,
}

impl TieredHistoryCompressor {
    pub fn new(token_counter: Arc<dyn TokenCounter>, config: HistoryCompressionConfig) -> Self {
        Self {
            token_counter,
            summary_provider: None,
            memory_scorer: None,
            config,
        }
    }

    pub fn with_summary_provider(mut self, provider: Arc<dyn SummaryProvider>) -> Self {
        self.summary_provider = Some(provider);
        self
    }

    pub fn with_memory_scorer(mut self, scorer: Arc<dyn MemoryScorer>) -> Self {
        self.memory_scorer = Some(scorer);
        self
    }

    pub fn config(&self) -> &HistoryCompressionConfig {
        &self.config
    }

    /// Compress conversation history using the tiered pipeline.
    ///
    /// `tier0_count` is the number of recent turns to keep verbatim
    /// (determined by DepthMode externally).
    pub async fn compress(
        &self,
        history: &[Message],
        _budget_tokens: usize,
        tier0_count: usize,
    ) -> CompressedHistory {
        // Quick turn count estimate for early exit (avoids microcompaction + grouping)
        let estimated_turns = count_user_messages(history);
        if estimated_turns <= tier0_count {
            return CompressedHistory {
                summaries: vec![],
                recent_messages: history.to_vec(),
                preamble: vec![],
                total_tokens: history
                    .iter()
                    .map(|m| token_counter::estimate_message_tokens(&*self.token_counter, m))
                    .sum(),
            };
        }

        // Step 0: Microcompact stale tool results
        let history = microcompact_tool_results(history.to_vec(), tier0_count * 2);

        // Step 1: Group into turns
        let (preamble, mut turns) = group_into_turns(&history, &*self.token_counter);

        // Step 2: Split into Tier 0 (recent) and older turns
        let tier0_start = turns.len().saturating_sub(tier0_count);
        let (older_turns, recent_turns) = turns.split_at_mut(tier0_start);

        // Collect Tier 0 messages
        let recent_messages: Vec<Message> = recent_turns
            .iter()
            .flat_map(|t| t.messages.iter().cloned())
            .collect();

        // Step 3: Score older turns (if cognitive scoring enabled)
        if self.config.use_cognitive_scoring {
            if let Some(scorer) = &self.memory_scorer {
                let texts: Vec<String> = older_turns.iter().map(|t| t.scoring_content()).collect();
                let scores = scorer.score_batch(&texts).await;
                for (turn, score) in older_turns.iter_mut().zip(scores) {
                    turn.cognitive_score = Some(score);
                }
            }
        }

        // Step 4: Assign tiers
        self.assign_tiers(older_turns, tier0_start);

        // Step 5: Compress each tier
        let summaries = self.compress_turns(older_turns).await;

        let preamble_tokens: usize = preamble
            .iter()
            .map(|m| token_counter::estimate_message_tokens(&*self.token_counter, m))
            .sum();
        let recent_tokens: usize = recent_messages
            .iter()
            .map(|m| token_counter::estimate_message_tokens(&*self.token_counter, m))
            .sum();
        let summary_tokens: usize = summaries.iter().map(|s| s.token_count).sum();

        CompressedHistory {
            summaries,
            recent_messages,
            preamble,
            total_tokens: preamble_tokens + summary_tokens + recent_tokens,
        }
    }

    /// Compress with delta optimization: reuse existing prefix, only compress new messages.
    pub async fn compress_with_delta(
        &self,
        history: &[Message],
        budget_tokens: usize,
        tier0_count: usize,
        existing_prefix: &[TierSummary],
        compressed_through_idx: usize,
    ) -> CompressedHistory {
        // If no existing prefix or index is invalid, fall back to full compression
        if existing_prefix.is_empty() || compressed_through_idx >= history.len() {
            return self.compress(history, budget_tokens, tier0_count).await;
        }

        // Delta: only new messages since last compression
        let delta_messages = &history[compressed_through_idx..];

        // Compress only the delta
        let delta_result = self
            .compress(delta_messages, budget_tokens, tier0_count)
            .await;

        // Merge existing prefix with delta summaries
        let mut merged_summaries = Vec::new();

        // Cheap turn count for demotion distance calculation
        let total_turns = count_user_messages(history);

        // Process existing summaries: keep or demote
        for summary in existing_prefix {
            let distance_from_end = total_turns.saturating_sub(summary.turn_range.1);
            let score = summary.cognitive_score.unwrap_or(0.0);

            if summary.tier == CompressionTier::Detailed
                && distance_from_end > self.config.tier1_demotion_threshold
                && score < self.config.high_relevance_threshold
            {
                // Demote: re-compress the Tier 1 text as Tier 2
                let demoted_content = if let Some(provider) = &self.summary_provider {
                    let segments = vec![vec![Message::user(&summary.content)]];
                    let results = provider
                        .summarize_batch(segments, CompressionTier::Condensed)
                        .await;
                    results
                        .into_iter()
                        .next()
                        .and_then(|r| r.ok())
                        .unwrap_or_else(|| summary.content.clone())
                } else {
                    first_snippet(&summary.content, 100)
                };

                let token_count = self.token_counter.estimate_text(&demoted_content);
                merged_summaries.push(TierSummary {
                    tier: CompressionTier::Condensed,
                    content: demoted_content,
                    turn_range: summary.turn_range,
                    token_count,
                    cognitive_score: summary.cognitive_score,
                });
            } else {
                // Keep as-is
                merged_summaries.push(summary.clone());
            }
        }

        // Append delta summaries
        merged_summaries.extend(delta_result.summaries);

        let merged_tokens: usize = merged_summaries.iter().map(|s| s.token_count).sum();
        let recent_tokens: usize = delta_result
            .recent_messages
            .iter()
            .map(|m| token_counter::estimate_message_tokens(&*self.token_counter, m))
            .sum();
        let preamble_tokens: usize = delta_result
            .preamble
            .iter()
            .map(|m| token_counter::estimate_message_tokens(&*self.token_counter, m))
            .sum();

        CompressedHistory {
            summaries: merged_summaries,
            recent_messages: delta_result.recent_messages,
            preamble: delta_result.preamble,
            total_tokens: merged_tokens + recent_tokens + preamble_tokens,
        }
    }

    /// Assign tiers to older turns based on cognitive score + recency.
    fn assign_tiers(&self, turns: &mut [ConversationTurn], total_turns: usize) {
        let demotion_threshold = self.config.tier1_demotion_threshold;
        let high = self.config.high_relevance_threshold;
        let low = self.config.low_relevance_threshold;

        for turn in turns.iter_mut() {
            let distance_from_end = total_turns - turn.turn_index;
            let score = turn.cognitive_score.unwrap_or(0.0);

            let tier = if score >= high {
                // Cognitive promotion: high-relevance turns stay detailed
                AssignedTier::Detailed
            } else if distance_from_end <= demotion_threshold || score >= low {
                // Within recency window or moderate relevance
                AssignedTier::Detailed
            } else {
                AssignedTier::Condensed
            };

            turn.assigned_tier = Some(tier);
        }
    }

    /// Compress turns according to their assigned tier.
    async fn compress_turns(&self, turns: &[ConversationTurn]) -> Vec<TierSummary> {
        let mut summaries = Vec::new();

        // Group consecutive turns by their assigned tier for batching
        let mut batch_start = 0;
        while batch_start < turns.len() {
            let tier = turns[batch_start]
                .assigned_tier
                .unwrap_or(AssignedTier::Condensed);
            let mut batch_end = batch_start + 1;
            while batch_end < turns.len()
                && turns[batch_end]
                    .assigned_tier
                    .unwrap_or(AssignedTier::Condensed)
                    == tier
            {
                batch_end += 1;
            }

            let batch = &turns[batch_start..batch_end];
            let compression_tier = match tier {
                AssignedTier::Detailed => CompressionTier::Detailed,
                AssignedTier::Condensed => CompressionTier::Condensed,
                AssignedTier::Verbatim => {
                    batch_start = batch_end;
                    continue;
                }
            };

            // Process in sub-batches of 5 (matching MAX_SEGMENTS_PER_CALL)
            for chunk in batch.chunks(5) {
                let chunk_summaries = self.compress_chunk(chunk, compression_tier).await;
                summaries.extend(chunk_summaries);
            }

            batch_start = batch_end;
        }

        summaries
    }

    /// Compress a chunk of turns at a given tier.
    ///
    /// Uses hybrid extractive-first: if extractive fits the budget, skip LLM.
    async fn compress_chunk(
        &self,
        turns: &[ConversationTurn],
        tier: CompressionTier,
    ) -> Vec<TierSummary> {
        let snippet_len = match tier {
            CompressionTier::Detailed => DEFAULT_SNIPPET_LENGTH,
            CompressionTier::Condensed => 100,
        };

        let final_texts: Vec<String> = if let Some(provider) = &self.summary_provider {
            let segments: Vec<Vec<Message>> = turns.iter().map(|t| t.messages.clone()).collect();
            let results = provider.summarize_batch(segments, tier).await;

            // Use LLM result if successful, compute extractive only as fallback
            results
                .into_iter()
                .enumerate()
                .map(|(i, result)| match result {
                    Ok(text) if !text.is_empty() => text,
                    _ => extractive_turn_summary(&turns[i].messages, snippet_len),
                })
                .collect()
        } else {
            turns
                .iter()
                .map(|t| extractive_turn_summary(&t.messages, snippet_len))
                .collect()
        };

        // Build TierSummary for each turn
        final_texts
            .into_iter()
            .zip(turns.iter())
            .map(|(content, turn)| {
                let token_count = self.token_counter.estimate_text(&content);
                TierSummary {
                    tier,
                    content,
                    turn_range: (turn.turn_index, turn.turn_index + 1),
                    token_count,
                    cognitive_score: turn.cognitive_score,
                }
            })
            .collect()
    }
}

/// Create an extractive summary for a single turn's messages.
fn extractive_turn_summary(messages: &[Message], snippet_len: usize) -> String {
    let mut lines = Vec::new();
    for msg in messages {
        match msg {
            Message::User { content } => {
                let text = match content {
                    providers::UserContent::Text(t) => t.clone(),
                    providers::UserContent::MultiPart(_) => "[multipart]".to_string(),
                };
                lines.push(format!("User: {}", first_snippet(&text, snippet_len)));
            }
            Message::Assistant {
                content: Some(text),
                ..
            } => {
                lines.push(format!("Assistant: {}", first_snippet(text, snippet_len)));
            }
            Message::Tool { name, content, .. } => {
                lines.push(format!(
                    "{}: {}",
                    name,
                    first_snippet(content, snippet_len / 2)
                ));
            }
            _ => {}
        }
    }
    lines.join("\n")
}

/// Count user messages as a cheap proxy for turn count.
fn count_user_messages(messages: &[Message]) -> usize {
    messages
        .iter()
        .filter(|m| matches!(m, Message::User { .. }))
        .count()
}

/// Tool names eligible for microcompaction (verbose output tools).
const COMPACTABLE_TOOLS: &[&str] = &[
    "read_file",
    "bash",
    "grep",
    "glob",
    "web_search",
    "web_fetch",
];

/// Minimum token count for a tool result to be worth compacting.
const MIN_COMPACTABLE_TOKENS: usize = 50;

/// Snippet length for compacted tool results.
const MICROCOMPACT_SNIPPET_LEN: usize = 150;

/// Pre-pass: compact stale tool results in older messages before tiered compression.
///
/// Tool results outside the `recent_window` (counted from the end) that match
/// `COMPACTABLE_TOOLS` and exceed `MIN_COMPACTABLE_TOKENS` get replaced with
/// a snippet summary.
fn microcompact_tool_results(mut messages: Vec<Message>, recent_window: usize) -> Vec<Message> {
    if messages.len() <= recent_window {
        return messages;
    }

    let cutoff = messages.len().saturating_sub(recent_window);

    for msg in messages[..cutoff].iter_mut() {
        if let Message::Tool { name, content, .. } = msg {
            if COMPACTABLE_TOOLS.iter().any(|t| name.contains(t))
                && content.len() > MIN_COMPACTABLE_TOKENS * 4
            {
                let original_len = content.len();
                let snippet = first_snippet(content, MICROCOMPACT_SNIPPET_LEN);
                *content = format!(
                    "{} [compressed {} result, originally {} chars]",
                    snippet, name, original_len
                );
            }
        }
    }

    messages
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token_counter::default_token_counter;
    use async_trait::async_trait;

    struct MockSummaryProvider {
        response: String,
    }

    #[async_trait]
    impl SummaryProvider for MockSummaryProvider {
        async fn summarize_batch(
            &self,
            segments: Vec<Vec<Message>>,
            _tier: CompressionTier,
        ) -> Vec<Result<String, String>> {
            segments.iter().map(|_| Ok(self.response.clone())).collect()
        }
    }

    struct MockMemoryScorer {
        scores: Vec<f64>,
    }

    #[async_trait]
    impl MemoryScorer for MockMemoryScorer {
        async fn score_batch(&self, texts: &[String]) -> Vec<f64> {
            texts
                .iter()
                .enumerate()
                .map(|(i, _)| self.scores.get(i).copied().unwrap_or(0.5))
                .collect()
        }
    }

    fn default_config() -> HistoryCompressionConfig {
        HistoryCompressionConfig::default()
    }

    fn make_history(n: usize) -> Vec<Message> {
        let mut msgs = Vec::new();
        for i in 0..n {
            msgs.push(Message::user(format!("User message {}", i)));
            msgs.push(Message::assistant(format!("Assistant response {}", i)));
        }
        msgs
    }

    #[tokio::test]
    async fn test_short_session_no_compression() {
        let compressor = TieredHistoryCompressor::new(default_token_counter(), default_config());
        let history = make_history(3); // 3 turns, tier0_count = 8
        let result = compressor.compress(&history, 50_000, 8).await;
        assert!(result.summaries.is_empty());
        assert_eq!(result.recent_messages.len(), 6); // all kept verbatim
    }

    #[tokio::test]
    async fn test_tiered_compression_produces_summaries() {
        let provider = Arc::new(MockSummaryProvider {
            response: "LLM summary of turn".into(),
        });
        let compressor = TieredHistoryCompressor::new(default_token_counter(), default_config())
            .with_summary_provider(provider);

        let history = make_history(20); // 20 turns
        let result = compressor.compress(&history, 50_000, 8).await;

        assert!(
            !result.summaries.is_empty(),
            "should have compressed older turns"
        );
        assert!(
            !result.recent_messages.is_empty(),
            "should keep recent verbatim"
        );

        // Verify summaries contain LLM output
        assert!(
            result
                .summaries
                .iter()
                .any(|s| s.content.contains("LLM summary")),
            "expected LLM summary in results"
        );
    }

    #[tokio::test]
    async fn test_cognitive_promotion_to_tier1() {
        // Old turn (index 0) with high score should be Tier 1
        let scorer = Arc::new(MockMemoryScorer {
            scores: vec![0.9, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2],
        });
        let provider = Arc::new(MockSummaryProvider {
            response: "summary".into(),
        });

        let compressor = TieredHistoryCompressor::new(default_token_counter(), default_config())
            .with_summary_provider(provider)
            .with_memory_scorer(scorer);

        let history = make_history(20); // 20 turns, tier0 = 8, so 12 older
        let result = compressor.compress(&history, 50_000, 8).await;

        // The first turn (score 0.9 > 0.7) should be Detailed
        let first_summary = result.summaries.iter().find(|s| s.turn_range.0 == 0);
        assert!(first_summary.is_some());
        assert_eq!(first_summary.unwrap().tier, CompressionTier::Detailed);
    }

    #[tokio::test]
    async fn test_no_scorer_falls_back_to_recency() {
        let provider = Arc::new(MockSummaryProvider {
            response: "summary".into(),
        });
        let compressor = TieredHistoryCompressor::new(default_token_counter(), default_config())
            .with_summary_provider(provider);

        let history = make_history(20);
        let result = compressor.compress(&history, 50_000, 8).await;

        // Without scorer, should still produce summaries (recency-based)
        assert!(!result.summaries.is_empty());
    }

    #[tokio::test]
    async fn test_extractive_fallback_on_no_provider() {
        let compressor = TieredHistoryCompressor::new(default_token_counter(), default_config());
        let history = make_history(20);
        let result = compressor.compress(&history, 50_000, 8).await;

        // Should produce extractive summaries
        assert!(!result.summaries.is_empty());
        assert!(
            result.summaries.iter().any(|s| s.content.contains("User:")),
            "extractive fallback should contain 'User:' prefix"
        );
    }

    #[tokio::test]
    async fn test_preamble_system_messages_preserved() {
        let mut history = vec![
            Message::system("You are a helpful assistant."),
            Message::system("Extra context."),
        ];
        history.extend(make_history(20));

        let compressor = TieredHistoryCompressor::new(default_token_counter(), default_config());
        let result = compressor.compress(&history, 50_000, 8).await;

        assert_eq!(result.preamble.len(), 2);
    }

    #[test]
    fn test_extractive_turn_summary() {
        let messages = vec![
            Message::user("What is the weather?"),
            Message::assistant("It's sunny today."),
        ];
        let summary = extractive_turn_summary(&messages, 200);
        assert!(summary.contains("User: What is the weather?"));
        assert!(summary.contains("Assistant: It's sunny today."));
    }

    #[test]
    fn test_scoring_content() {
        let turn = ConversationTurn {
            messages: vec![
                Message::user("How do I deploy?"),
                Message::assistant("Use docker compose up."),
            ],
            turn_index: 0,
            token_count: 10,
            cognitive_score: None,
            assigned_tier: None,
        };
        let content = turn.scoring_content();
        assert!(content.contains("User: How do I deploy?"));
        assert!(content.contains("Assistant: Use docker compose up."));
    }

    #[test]
    fn test_microcompact_stale_tool_results() {
        let messages = vec![
            Message::user("Read the file"),
            Message::Assistant {
                content: None,
                tool_calls: Some(vec![providers::ToolCallMessage {
                    id: "tc1".into(),
                    r#type: "function".into(),
                    function: providers::FunctionCall {
                        name: "read_file".into(),
                        arguments: "{}".into(),
                    },
                }]),
                reasoning_content: None,
            },
            Message::Tool {
                tool_call_id: "tc1".into(),
                name: "read_file".into(),
                content: "A".repeat(5000), // large tool result
            },
            Message::assistant("Here's what I found."),
            Message::user("Now do something else"),
            Message::assistant("Sure."),
        ];

        let compacted = microcompact_tool_results(messages, 8); // recent window = 8
                                                                // All within recent window (6 < 8), so nothing compacted
        if let Message::Tool { content, .. } = &compacted[2] {
            assert_eq!(
                content.len(),
                5000,
                "within recent window, should not compact"
            );
        }

        // Now test with smaller window
        let messages2 = vec![
            Message::user("Read the file"),
            Message::Assistant {
                content: None,
                tool_calls: Some(vec![providers::ToolCallMessage {
                    id: "tc1".into(),
                    r#type: "function".into(),
                    function: providers::FunctionCall {
                        name: "read_file".into(),
                        arguments: "{}".into(),
                    },
                }]),
                reasoning_content: None,
            },
            Message::Tool {
                tool_call_id: "tc1".into(),
                name: "read_file".into(),
                content: "A".repeat(5000),
            },
            Message::assistant("Here's what I found."),
            Message::user("Now do something else"),
            Message::assistant("Sure."),
        ];

        let compacted2 = microcompact_tool_results(messages2, 2); // only last 2 recent
        if let Message::Tool { content, .. } = &compacted2[2] {
            assert!(
                content.len() < 500,
                "stale tool result should be compacted, got {} chars",
                content.len()
            );
            assert!(content.contains("[compressed"));
        } else {
            panic!("expected Tool message at index 2");
        }
    }

    #[test]
    fn test_microcompact_preserves_recent_tool_results() {
        let messages = vec![
            Message::user("Do something"),
            Message::Tool {
                tool_call_id: "tc1".into(),
                name: "search".into(),
                content: "B".repeat(5000),
            },
            Message::assistant("Done."),
        ];

        // All messages within recent window — nothing compacted
        let compacted = microcompact_tool_results(messages, 8);
        if let Message::Tool { content, .. } = &compacted[1] {
            assert_eq!(
                content.len(),
                5000,
                "recent tool results should not be compacted"
            );
        }
    }

    // Delta compression tests

    #[tokio::test]
    async fn test_delta_compression_reuses_prefix() {
        let provider = Arc::new(MockSummaryProvider {
            response: "delta summary".into(),
        });
        let compressor = TieredHistoryCompressor::new(default_token_counter(), default_config())
            .with_summary_provider(provider);

        // Simulate existing prefix from previous compression
        let existing_prefix = vec![
            TierSummary {
                tier: CompressionTier::Detailed,
                content: "Old Tier 1 summary of turns 0-3".into(),
                turn_range: (0, 4),
                token_count: 20,
                cognitive_score: Some(0.5),
            },
            TierSummary {
                tier: CompressionTier::Condensed,
                content: "Old Tier 2 gist of turns 4-7".into(),
                turn_range: (4, 8),
                token_count: 10,
                cognitive_score: Some(0.3),
            },
        ];

        // Full history: 20 turns. Prefix covers first 8 turns (16 messages).
        let history = make_history(20);
        let compressed_through_idx = 16; // 8 turns * 2 msgs/turn

        let result = compressor
            .compress_with_delta(
                &history,
                50_000,
                8, // tier0_count
                &existing_prefix,
                compressed_through_idx,
            )
            .await;

        // Should include the old summaries + new delta summaries
        assert!(
            result.summaries.len() >= 2,
            "should include old + new summaries"
        );

        // Recent 8 turns should be verbatim
        assert!(!result.recent_messages.is_empty());
    }

    #[tokio::test]
    async fn test_delta_demotes_old_tier1() {
        let provider = Arc::new(MockSummaryProvider {
            response: "demoted summary".into(),
        });
        let mut config = default_config();
        config.tier1_demotion_threshold = 5; // aggressive demotion

        let compressor = TieredHistoryCompressor::new(default_token_counter(), config)
            .with_summary_provider(provider);

        // Old Tier 1 summary far from current end
        let existing_prefix = vec![TierSummary {
            tier: CompressionTier::Detailed,
            content: "Old detailed summary of turns 0-2".into(),
            turn_range: (0, 3),
            token_count: 30,
            cognitive_score: Some(0.4), // below high threshold
        }];

        let history = make_history(30); // 30 turns, far from turn 0-2
        let result = compressor
            .compress_with_delta(&history, 50_000, 8, &existing_prefix, 6)
            .await;

        // The old Tier 1 should be demoted to Tier 2
        let old_summary = result.summaries.iter().find(|s| s.turn_range == (0, 3));
        assert!(old_summary.is_some());
        assert_eq!(
            old_summary.unwrap().tier,
            CompressionTier::Condensed,
            "old Tier 1 should be demoted to Condensed"
        );
    }
}
