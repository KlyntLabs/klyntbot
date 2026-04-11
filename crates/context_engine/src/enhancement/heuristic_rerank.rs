use std::collections::HashSet;

use async_trait::async_trait;

use crate::memory_retriever::MemoryEntry;

use super::prf::tokenize;
use super::traits::RankingStage;
use super::types::{EnhancementBudget, QueryBundle, QuerySource};

// ---------------------------------------------------------------------------
// HeuristicRerankConfig
// ---------------------------------------------------------------------------

/// Configuration for the heuristic rerank stage.
#[derive(Debug, Clone)]
pub struct HeuristicRerankConfig {
    /// Weight applied to query-term overlap when boosting candidate scores.
    pub term_overlap_weight: f64,
}

impl Default for HeuristicRerankConfig {
    fn default() -> Self {
        Self {
            term_overlap_weight: 0.05,
        }
    }
}

// ---------------------------------------------------------------------------
// HeuristicRerankStage
// ---------------------------------------------------------------------------

/// Re-ranking stage that boosts candidates based on query-term overlap.
///
/// For each candidate, computes the fraction of query tokens that appear in
/// the candidate's content, then adds a weighted boost to the candidate's
/// score. Zero LLM cost — purely lexical.
pub struct HeuristicRerankStage {
    config: HeuristicRerankConfig,
}

impl HeuristicRerankStage {
    /// Create a new heuristic rerank stage with the given config.
    pub fn new(config: HeuristicRerankConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl RankingStage for HeuristicRerankStage {
    fn name(&self) -> QuerySource {
        QuerySource::HeuristicRerank
    }

    async fn rerank(
        &self,
        query: &QueryBundle,
        mut candidates: Vec<MemoryEntry>,
        _budget: &EnhancementBudget,
    ) -> common::Result<Vec<MemoryEntry>> {
        if candidates.is_empty() {
            return Ok(candidates);
        }

        // Tokenize the primary query into a set for O(1) lookup.
        let query_terms: HashSet<String> = tokenize(&query.primary).into_iter().collect();

        if query_terms.is_empty() {
            return Ok(candidates);
        }

        let query_term_count = query_terms.len() as f64;

        for entry in &mut candidates {
            let content_terms: HashSet<String> = tokenize(&entry.content).into_iter().collect();
            let intersection_count = query_terms.intersection(&content_terms).count() as f64;
            let overlap_ratio = intersection_count / query_term_count;
            let boost = overlap_ratio * self.config.term_overlap_weight;
            entry.score = (entry.score + boost).min(1.0);
        }

        // Sort by score descending.
        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(candidates)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use crate::memory_retriever::{MemoryEntry, MemorySource};

    fn make_entry(id: &str, content: &str, score: f64) -> MemoryEntry {
        MemoryEntry {
            id: id.to_string(),
            content: content.to_string(),
            score,
            source: MemorySource::CognitiveFact,
            raw_score: score,
        }
    }

    fn make_query(primary: &str) -> QueryBundle {
        QueryBundle::passthrough(primary)
    }

    fn default_budget() -> EnhancementBudget {
        EnhancementBudget::normal()
    }

    #[tokio::test]
    async fn test_heuristic_rerank_boosts_overlapping_terms() {
        let stage = HeuristicRerankStage::new(HeuristicRerankConfig::default());
        let query = make_query("budget finance quarterly");
        let budget = default_budget();

        let candidates = vec![
            // This one shares "finance" and "quarterly" with the query.
            make_entry("a", "finance quarterly planning meeting", 0.7),
            // This one shares no query terms.
            make_entry("b", "unrelated content about weather", 0.7),
        ];

        let result = stage.rerank(&query, candidates, &budget).await.unwrap();

        assert_eq!(result.len(), 2);
        // Entry "a" should sort first because it got a boost from overlapping terms.
        assert_eq!(result[0].id, "a", "overlapping entry should sort first");
        assert_eq!(result[1].id, "b", "non-overlapping entry should sort last");
        assert!(
            result[0].score > result[1].score,
            "overlapping entry score ({}) should exceed non-overlapping ({})",
            result[0].score,
            result[1].score
        );
    }

    #[tokio::test]
    async fn test_heuristic_rerank_score_capped_at_1() {
        let stage = HeuristicRerankStage::new(HeuristicRerankConfig {
            // Very high weight to force overflow beyond 1.0.
            term_overlap_weight: 10.0,
        });
        let query = make_query("budget finance quarterly planning");
        let budget = default_budget();

        // Entry with a near-perfect initial score that also overlaps all query terms.
        let candidates = vec![make_entry(
            "a",
            "budget finance quarterly planning review",
            0.98,
        )];

        let result = stage.rerank(&query, candidates, &budget).await.unwrap();

        assert_eq!(result.len(), 1);
        assert!(
            result[0].score <= 1.0,
            "score must not exceed 1.0, got {}",
            result[0].score
        );
    }

    #[tokio::test]
    async fn test_heuristic_rerank_empty_candidates() {
        let stage = HeuristicRerankStage::new(HeuristicRerankConfig::default());
        let query = make_query("budget finance");
        let budget = default_budget();

        let result = stage.rerank(&query, vec![], &budget).await.unwrap();

        assert!(result.is_empty(), "empty input should produce empty output");
    }
}
