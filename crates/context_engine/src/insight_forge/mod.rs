pub mod circuit_breaker;
pub mod decomposer;
pub mod domain_searcher;
pub mod llm_decomposer;

pub use circuit_breaker::CircuitBreaker;
pub use decomposer::{FallbackDecomposer, HeuristicDecomposer, QueryDecomposer};
pub use domain_searcher::DomainSearcher;
pub use llm_decomposer::{DecomposerLlm, LlmDecomposer};

use std::collections::HashMap;
use std::sync::Arc;

use tracing::{debug, warn};

use crate::assembler::ExecutionStrategy;
use crate::memory_retriever::{MemoryEntry, MemoryRetriever};

/// Configuration for the InsightForge multi-dimensional retrieval engine.
pub struct InsightForgeConfig {
    /// Whether InsightForge is enabled at all.
    pub enabled: bool,
    /// Maximum number of sub-queries produced by the decomposer.
    pub max_sub_queries: usize,
    /// Maximum results per domain source per sub-query.
    pub per_source_limit: usize,
    /// Hard cap on total returned entries.
    pub total_limit: usize,
    /// Timeout (ms) for each individual domain source search.
    pub per_source_timeout_ms: u64,
    /// Timeout (ms) for the decomposer step.
    pub decomposer_timeout_ms: u64,
    /// Number of failures before the per-session circuit breaker trips.
    pub circuit_breaker_threshold: u32,
    /// Seconds before a tripped circuit breaker resets.
    pub circuit_breaker_cooldown_secs: u64,
}

impl Default for InsightForgeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_sub_queries: 5,
            per_source_limit: 5,
            total_limit: 15,
            per_source_timeout_ms: 800,
            decomposer_timeout_ms: 2000,
            circuit_breaker_threshold: 3,
            circuit_breaker_cooldown_secs: 300,
        }
    }
}

/// Multi-dimensional retrieval orchestrator.
///
/// Decomposes a user query into sub-queries, fans out searches across the
/// memory retriever and any registered domain searchers, then merges results
/// via Reciprocal Rank Fusion (RRF).
pub struct InsightForge {
    config: InsightForgeConfig,
    decomposer: Arc<dyn QueryDecomposer>,
    memory_retriever: Arc<dyn MemoryRetriever>,
    searchers: Vec<Arc<dyn DomainSearcher>>,
    circuit_breaker: CircuitBreaker,
}

impl InsightForge {
    /// Create a new InsightForge orchestrator.
    pub fn new(
        config: InsightForgeConfig,
        decomposer: Arc<dyn QueryDecomposer>,
        memory_retriever: Arc<dyn MemoryRetriever>,
    ) -> Self {
        let circuit_breaker = CircuitBreaker::new(
            config.circuit_breaker_threshold,
            config.circuit_breaker_cooldown_secs,
        );
        Self {
            config,
            decomposer,
            memory_retriever,
            searchers: Vec::new(),
            circuit_breaker,
        }
    }

    /// Register a domain searcher.  Must be called before the `InsightForge`
    /// is wrapped in an `Arc`.
    pub fn add_searcher(&mut self, searcher: Arc<dyn DomainSearcher>) {
        self.searchers.push(searcher);
    }

    /// Decide whether InsightForge should activate for this request.
    pub fn should_activate(&self, strategy: &ExecutionStrategy, message: &str) -> bool {
        if !self.config.enabled {
            return false;
        }
        if matches!(strategy, ExecutionStrategy::Clarification { .. }) {
            return false;
        }
        if message.len() < 20 {
            return false;
        }
        true
    }

    /// Retrieve context entries via multi-dimensional search.
    ///
    /// 1. Checks the circuit breaker — falls back to plain memory retrieval if open.
    /// 2. Decomposes the query with a timeout — falls back on timeout.
    /// 3. For each sub-query, searches all sources in parallel (each with a per-source timeout).
    /// 4. Merges results via RRF, deduplicates, re-normalises scores to 0.0–1.0.
    pub async fn retrieve(
        &self,
        query: &str,
        total_limit: usize,
        session_key: Option<&str>,
    ) -> Vec<MemoryEntry> {
        let limit = total_limit.min(self.config.total_limit);

        // 1. Circuit breaker check.
        if let Some(sk) = session_key {
            if self.circuit_breaker.is_open(sk) {
                debug!("InsightForge circuit open for session, falling back");
                return self.fallback(query, limit).await;
            }
        }

        // 2. Decompose with timeout.
        let sub_queries = {
            let decompose_fut = self.decomposer.decompose(query, None);
            let timeout = tokio::time::timeout(
                std::time::Duration::from_millis(self.config.decomposer_timeout_ms),
                decompose_fut,
            );
            match timeout.await {
                Ok(subs) => {
                    let mut subs = subs;
                    subs.truncate(self.config.max_sub_queries);
                    subs
                }
                Err(_) => {
                    warn!("InsightForge decomposer timed out, falling back");
                    if let Some(sk) = session_key {
                        self.circuit_breaker.record_failure(sk);
                    }
                    return self.fallback(query, limit).await;
                }
            }
        };

        // 3. Fan-out: for each sub-query, search all sources in parallel.
        let per_source_limit = self.config.per_source_limit;
        let per_source_timeout =
            std::time::Duration::from_millis(self.config.per_source_timeout_ms);

        let mut all_ranked_lists: Vec<Vec<MemoryEntry>> = Vec::new();

        for sub_query in &sub_queries {
            let mut handles = Vec::new();

            // Memory retriever search.
            {
                let retriever = Arc::clone(&self.memory_retriever);
                let q = sub_query.clone();
                let timeout_dur = per_source_timeout;
                handles.push(tokio::spawn(async move {
                    tokio::time::timeout(timeout_dur, retriever.retrieve(&q, per_source_limit))
                        .await
                        .unwrap_or_default()
                }));
            }

            // Domain searcher searches.
            for searcher in &self.searchers {
                let s = Arc::clone(searcher);
                let q = sub_query.clone();
                let timeout_dur = per_source_timeout;
                handles.push(tokio::spawn(async move {
                    tokio::time::timeout(timeout_dur, s.search(&q, per_source_limit))
                        .await
                        .unwrap_or_default()
                }));
            }

            // Collect results from all sources for this sub-query.
            let mut sub_query_results: Vec<MemoryEntry> = Vec::new();
            for handle in handles {
                match handle.await {
                    Ok(entries) => sub_query_results.extend(entries),
                    Err(e) => {
                        warn!("InsightForge search task panicked: {e}");
                    }
                }
            }

            all_ranked_lists.push(sub_query_results);
        }

        // 4. RRF merge across sub-queries.
        let merged = self.rrf_merge(&all_ranked_lists, limit);

        // Budget allocation: ensure no single source provides more than 60% of results.
        let max_per_source = (limit as f64 * 0.6).ceil() as usize;
        let mut source_counts: HashMap<String, usize> = HashMap::new();
        let mut budgeted = Vec::new();
        let mut overflow = Vec::new();

        for entry in merged {
            let source_key = match &entry.source {
                crate::MemorySource::CognitiveFact => "cognitive".to_string(),
                crate::MemorySource::ConversationRecall => "recall".to_string(),
                crate::MemorySource::Domain { name } => name.clone(),
            };
            let count = source_counts.entry(source_key).or_insert(0);
            if *count < max_per_source {
                *count += 1;
                budgeted.push(entry);
            } else {
                overflow.push(entry);
            }
        }

        // Fill remaining slots from overflow (highest score first, already sorted)
        let remaining = limit.saturating_sub(budgeted.len());
        budgeted.extend(overflow.into_iter().take(remaining));
        budgeted.truncate(limit);

        budgeted
    }

    /// Reciprocal Rank Fusion merge.
    ///
    /// For each ranked list, each item receives `1 / (k + rank + 1)` where
    /// `k = 60`.  Items appearing in multiple lists accumulate scores.
    /// The result is sorted by score descending, deduplicated by ID, and
    /// re-normalised so the top score is 1.0.
    fn rrf_merge(&self, ranked_lists: &[Vec<MemoryEntry>], limit: usize) -> Vec<MemoryEntry> {
        const K: f64 = 60.0;

        // Accumulate scores per ID.
        let mut score_map: HashMap<String, f64> = HashMap::new();
        let mut entry_map: HashMap<String, MemoryEntry> = HashMap::new();

        for list in ranked_lists {
            for (rank, entry) in list.iter().enumerate() {
                let rrf_score = 1.0 / (K + rank as f64 + 1.0);
                *score_map.entry(entry.id.clone()).or_insert(0.0) += rrf_score;
                // Keep the entry with the highest original score for each ID.
                entry_map
                    .entry(entry.id.clone())
                    .and_modify(|existing| {
                        if entry.score > existing.score {
                            *existing = entry.clone();
                        }
                    })
                    .or_insert_with(|| entry.clone());
            }
        }

        // Sort by accumulated RRF score descending.
        let mut scored: Vec<(String, f64)> = score_map.into_iter().collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);

        // Re-normalise scores to 0.0–1.0.
        let max_score = scored.first().map(|(_, s)| *s).unwrap_or(1.0);
        let normaliser = if max_score > 0.0 { max_score } else { 1.0 };

        scored
            .into_iter()
            .filter_map(|(id, rrf_score)| {
                entry_map.remove(&id).map(|mut e| {
                    e.score = rrf_score / normaliser;
                    e
                })
            })
            .collect()
    }

    /// Fallback: plain memory retrieval without decomposition or domain search.
    async fn fallback(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        self.memory_retriever.retrieve(query, limit).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory_retriever::{MemoryEntry, MemoryRetriever, MemorySource};
    use async_trait::async_trait;

    struct MockRetriever {
        entries: Vec<MemoryEntry>,
    }

    #[async_trait]
    impl MemoryRetriever for MockRetriever {
        async fn retrieve(&self, _query: &str, limit: usize) -> Vec<MemoryEntry> {
            self.entries.iter().take(limit).cloned().collect()
        }
    }

    fn make_entry(id: &str, score: f64) -> MemoryEntry {
        MemoryEntry {
            id: id.to_string(),
            content: format!("content for {}", id),
            score,
            source: MemorySource::CognitiveFact,
            raw_score: score,
        }
    }

    #[tokio::test]
    async fn test_insight_forge_retrieve_with_heuristic() {
        let retriever = Arc::new(MockRetriever {
            entries: vec![make_entry("m1", 0.9), make_entry("m2", 0.7)],
        });
        let decomposer = Arc::new(HeuristicDecomposer);

        let forge = InsightForge::new(InsightForgeConfig::default(), decomposer, retriever);

        let results = forge
            .retrieve(
                "Help me plan the API migration project",
                10,
                Some("test-session"),
            )
            .await;

        assert!(!results.is_empty(), "Should return results");
        for entry in &results {
            assert!(entry.score <= 1.0, "Score {} should be <= 1.0", entry.score);
            assert!(entry.score >= 0.0, "Score {} should be >= 0.0", entry.score);
        }
    }

    #[test]
    fn test_should_activate() {
        let retriever = Arc::new(MockRetriever { entries: vec![] });
        let decomposer = Arc::new(HeuristicDecomposer);
        let forge = InsightForge::new(InsightForgeConfig::default(), decomposer, retriever);

        // Short message → false
        assert!(!forge.should_activate(&ExecutionStrategy::DirectResponse, "hi"));

        // Clarification → false
        assert!(!forge.should_activate(
            &ExecutionStrategy::Clarification {
                reason: "test".into()
            },
            "This is a long enough clarification message"
        ));

        // ToolAssisted + long message → true
        assert!(forge.should_activate(
            &ExecutionStrategy::ToolAssisted { max_iterations: 5 },
            "Help me plan the API migration project"
        ));

        // DirectResponse + long message → true
        assert!(forge.should_activate(
            &ExecutionStrategy::DirectResponse,
            "Tell me about the API migration project status"
        ));
    }

    #[test]
    fn test_rrf_merge_deduplicates() {
        let retriever = Arc::new(MockRetriever { entries: vec![] });
        let decomposer = Arc::new(HeuristicDecomposer);
        let forge = InsightForge::new(InsightForgeConfig::default(), decomposer, retriever);

        let list1 = vec![make_entry("a", 0.9), make_entry("b", 0.7)];
        let list2 = vec![make_entry("a", 0.8), make_entry("c", 0.6)];

        let merged = forge.rrf_merge(&[list1, list2], 10);

        // "a" appears in both lists → should rank highest.
        assert_eq!(merged[0].id, "a", "Item in both lists should rank first");

        // 3 unique items total.
        assert_eq!(merged.len(), 3, "Should have 3 unique entries");

        // Top score should be 1.0 after re-normalisation.
        assert!(
            (merged[0].score - 1.0).abs() < f64::EPSILON,
            "Top score should be 1.0"
        );
    }
}
