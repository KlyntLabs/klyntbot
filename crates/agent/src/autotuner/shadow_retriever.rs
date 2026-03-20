//! Implements [`ShadowRetriever`] by delegating to [`UnifiedMemoryService`]
//! with trial parameter overrides for shadow memory retrieval.

use std::sync::Arc;

use async_trait::async_trait;
use autotuner::{ShadowContext, ShadowRetriever, ShadowRetrievalResult};
use common::TrialParams;
use cognitive::UnifiedMemoryService;

/// Shadow retriever backed by the agent's [`UnifiedMemoryService`].
///
/// Runs memory retrieval with trial-specified `vector_top_k`, `min_similarity`,
/// and relevance weights so the autotuner can compare retrieval quality across
/// parameter variants without affecting live behaviour.
pub struct AgentShadowRetriever {
    memory_service: Arc<UnifiedMemoryService>,
    config_defaults: [f64; 6], // 6 default relevance weights from CognitiveConfig
}

impl AgentShadowRetriever {
    pub fn new(memory_service: Arc<UnifiedMemoryService>, config_defaults: [f64; 6]) -> Self {
        Self {
            memory_service,
            config_defaults,
        }
    }
}

#[async_trait]
impl ShadowRetriever for AgentShadowRetriever {
    async fn retrieve_shadow(
        &self,
        query: &str,
        _context: &ShadowContext,
        params: &TrialParams,
    ) -> common::Result<ShadowRetrievalResult> {
        let weights = params.resolve_relevance_weights(&self.config_defaults);
        let top_k = params.vector_top_k.unwrap_or(30);
        let min_sim = params.min_similarity.unwrap_or(0.55);

        let entries = self
            .memory_service
            .retrieve_with_overrides(query, top_k, min_sim, weights)
            .await?;

        let total = entries.len();
        let avg_score = if total > 0 {
            entries.iter().map(|e| e.score).sum::<f64>() / total as f64
        } else {
            0.0
        };

        // avg_age_days placeholder — real age computation needs fact timestamps
        // from the DB, which aren't exposed on MemoryEntry.
        let avg_age_days = 0.0;

        Ok(ShadowRetrievalResult {
            memory_ids: entries.into_iter().map(|e| e.id).collect(),
            avg_score,
            avg_age_days,
            total_retrieved: total,
        })
    }
}
