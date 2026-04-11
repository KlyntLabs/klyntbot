//! LLM pairwise relevance scoring for top-N retrieval results.
//!
//! The [`LlmRerankStage`] asks an LLM to rate each candidate's relevance to
//! the query on a 0–10 scale, then re-orders the top-N candidates by those
//! scores. Remaining candidates (beyond top-N) are appended unchanged.
//!
//! Only runs when `budget.max_llm_calls > 0` and a provider is configured.

use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use tracing::debug;

use context_engine::enhancement::{EnhancementBudget, QueryBundle, QuerySource, RankingStage};
use context_engine::memory_retriever::MemoryEntry;

// ---------------------------------------------------------------------------
// LlmRerankStage
// ---------------------------------------------------------------------------

/// Re-ranking stage that uses an LLM to score the top-N candidates.
///
/// Candidates beyond `top_n` are not scored and are appended after the
/// reranked results in their original order.
pub struct LlmRerankStage {
    provider: Option<providers::DynProvider>,
    model: Option<String>,
    top_n: usize,
}

impl LlmRerankStage {
    pub fn new(
        provider: Option<providers::DynProvider>,
        model: Option<String>,
        top_n: usize,
    ) -> Self {
        Self {
            provider,
            model,
            top_n,
        }
    }

    /// Build a prompt that asks the LLM to rate each candidate 0–10.
    fn build_prompt(query: &QueryBundle, candidates: &[MemoryEntry]) -> String {
        let mut prompt = format!(
            "Rate each item's relevance to the query on 0-10.\n\
             Query: \"{}\"\n\n\
             Items:\n",
            query.primary
        );
        for (i, entry) in candidates.iter().enumerate() {
            let preview: String = entry.content.chars().take(120).collect();
            prompt.push_str(&format!("{}. {}\n", i + 1, preview));
        }
        prompt.push_str("\nReturn scores as: ID:SCORE (one per line, nothing else)");
        prompt
    }

    /// Parse `ID:SCORE` lines from the LLM response into a map.
    fn parse_scores(text: &str) -> HashMap<String, f64> {
        let mut scores = HashMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some((id_part, score_part)) = line.split_once(':') {
                let id = id_part.trim().to_string();
                if let Ok(raw) = score_part.trim().parse::<f64>() {
                    let clamped = raw.clamp(0.0, 10.0);
                    scores.insert(id, clamped);
                }
            }
        }
        scores
    }
}

#[async_trait]
impl RankingStage for LlmRerankStage {
    fn name(&self) -> QuerySource {
        QuerySource::LlmRerank
    }

    async fn rerank(
        &self,
        query: &QueryBundle,
        candidates: Vec<MemoryEntry>,
        budget: &EnhancementBudget,
    ) -> common::Result<Vec<MemoryEntry>> {
        // Skip if budget forbids LLM calls.
        if budget.max_llm_calls == 0 {
            return Ok(candidates);
        }

        // Skip if no provider or nothing to rerank.
        let provider = match self.provider.as_ref() {
            Some(p) => p,
            None => return Ok(candidates),
        };
        if candidates.is_empty() {
            return Ok(candidates);
        }

        // Split into the top-N (to be reranked) and the remainder.
        let rerank_count = self.top_n.min(candidates.len());
        let mut to_rerank = candidates;
        let rest = to_rerank.split_off(rerank_count);

        let prompt = Self::build_prompt(query, &to_rerank);
        let messages = vec![providers::Message::user(prompt)];
        let model_name = self
            .model
            .clone()
            .unwrap_or_else(|| provider.default_model().to_string());
        let params = providers::ChatParams::new(model_name)
            .with_max_tokens(100)
            .with_temperature(0.0);

        let timeout_ms = budget.max_latency_ms.min(800);
        let timeout_dur = Duration::from_millis(timeout_ms);

        let result =
            tokio::time::timeout(timeout_dur, provider.chat(&messages, None, &params)).await;

        match result {
            Ok(Ok(response)) => {
                let text = response.content.as_deref().unwrap_or("").to_string();
                let scores = Self::parse_scores(&text);

                // Apply scores: candidate IDs are their 1-based position strings.
                for (i, entry) in to_rerank.iter_mut().enumerate() {
                    let key = (i + 1).to_string();
                    if let Some(&raw_score) = scores.get(&key) {
                        // Normalize 0–10 → 0.0–1.0
                        entry.score = raw_score / 10.0;
                    }
                }

                // Sort reranked slice by score descending.
                to_rerank.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                debug!(
                    query = query.primary.as_str(),
                    reranked = rerank_count,
                    "LlmRerankStage: reranked top-N candidates"
                );

                // Append unscored remainder and return.
                to_rerank.extend(rest);
                Ok(to_rerank)
            }
            Ok(Err(e)) => {
                debug!(error = %e, "LlmRerankStage: provider error, returning candidates unchanged");
                to_rerank.extend(rest);
                Ok(to_rerank)
            }
            Err(_) => {
                debug!("LlmRerankStage: timed out, returning candidates unchanged");
                to_rerank.extend(rest);
                Ok(to_rerank)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_scores_basic() {
        let text = "1:8\n2:3\n3:10\n";
        let scores = LlmRerankStage::parse_scores(text);
        assert_eq!(scores.get("1"), Some(&8.0));
        assert_eq!(scores.get("2"), Some(&3.0));
        assert_eq!(scores.get("3"), Some(&10.0));
    }

    #[test]
    fn parse_scores_clamps_out_of_range() {
        let text = "1:-5\n2:15\n";
        let scores = LlmRerankStage::parse_scores(text);
        assert_eq!(scores.get("1"), Some(&0.0));
        assert_eq!(scores.get("2"), Some(&10.0));
    }

    #[test]
    fn parse_scores_ignores_malformed_lines() {
        let text = "not a score\n1:7\nextra:stuff:here\n";
        let scores = LlmRerankStage::parse_scores(text);
        assert_eq!(scores.len(), 1);
        assert_eq!(scores.get("1"), Some(&7.0));
    }

    #[test]
    fn parse_scores_empty_input() {
        let scores = LlmRerankStage::parse_scores("");
        assert!(scores.is_empty());
    }

    #[test]
    fn build_prompt_contains_query_and_items() {
        use context_engine::memory_retriever::MemorySource;

        let query = QueryBundle::passthrough("budget review");
        let candidates = vec![
            MemoryEntry {
                id: "a".into(),
                content: "Quarterly budget review notes".into(),
                score: 0.8,
                source: MemorySource::CognitiveFact,
                raw_score: 0.8,
            },
            MemoryEntry {
                id: "b".into(),
                content: "Unrelated weather forecast".into(),
                score: 0.5,
                source: MemorySource::CognitiveFact,
                raw_score: 0.5,
            },
        ];

        let prompt = LlmRerankStage::build_prompt(&query, &candidates);
        assert!(prompt.contains("budget review"));
        assert!(prompt.contains("1. Quarterly budget review notes"));
        assert!(prompt.contains("2. Unrelated weather forecast"));
        assert!(prompt.contains("ID:SCORE"));
    }

    #[tokio::test]
    async fn rerank_skips_when_budget_zero_llm_calls() {
        let stage = LlmRerankStage::new(None, None, 5);
        let budget = EnhancementBudget::normal(); // max_llm_calls == 0

        use context_engine::memory_retriever::MemorySource;
        let candidates = vec![MemoryEntry {
            id: "x".into(),
            content: "some content".into(),
            score: 0.6,
            source: MemorySource::CognitiveFact,
            raw_score: 0.6,
        }];

        let query = QueryBundle::passthrough("test");
        let result = stage
            .rerank(&query, candidates.clone(), &budget)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "x");
    }

    #[tokio::test]
    async fn rerank_skips_when_no_provider() {
        let stage = LlmRerankStage::new(None, None, 5);
        let budget = EnhancementBudget::deep_think(); // max_llm_calls == 2

        use context_engine::memory_retriever::MemorySource;
        let candidates = vec![MemoryEntry {
            id: "x".into(),
            content: "some content".into(),
            score: 0.6,
            source: MemorySource::CognitiveFact,
            raw_score: 0.6,
        }];

        let query = QueryBundle::passthrough("test");
        let result = stage.rerank(&query, candidates, &budget).await.unwrap();
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn rerank_empty_candidates_returns_empty() {
        let stage = LlmRerankStage::new(None, None, 5);
        let budget = EnhancementBudget::deep_think();
        let query = QueryBundle::passthrough("test");
        let result = stage.rerank(&query, vec![], &budget).await.unwrap();
        assert!(result.is_empty());
    }
}
