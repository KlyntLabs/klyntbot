use async_trait::async_trait;
use common::TrialParams;
use serde::{Deserialize, Serialize};

#[async_trait]
pub trait ShadowClassifier: Send + Sync {
    async fn classify_shadow(
        &self,
        message: &str,
        context: &ShadowContext,
        params: &TrialParams,
    ) -> common::Result<ShadowPrediction>;
}

#[async_trait]
pub trait MetricSource: Send + Sync {
    async fn collect_metrics(
        &self,
        since: jiff::Timestamp,
        trial_id: Option<uuid::Uuid>,
    ) -> common::Result<MetricSnapshot>;
}

#[derive(Debug, Clone)]
pub struct ShadowContext {
    pub chat_id: String,
    pub session_key: String,
}

#[derive(Debug, Clone)]
pub struct ShadowPrediction {
    pub predicted_orchestrator: String,
    pub predicted_mode: String,
    pub confidence: f32,
    pub predicted_iteration_budget: u32,
    pub deferred_to_llm: bool,
}

/// Runs memory retrieval with trial parameter overrides for shadow scoring.
/// Returns a summary struct rather than raw MemoryEntry to avoid a dependency
/// from autotuner (L4) on cognitive (L5). The concrete AgentShadowRetriever
/// in the agent crate converts MemoryEntry → ShadowRetrievalResult.
#[async_trait]
pub trait ShadowRetriever: Send + Sync {
    async fn retrieve_shadow(
        &self,
        query: &str,
        context: &ShadowContext,
        params: &TrialParams,
    ) -> common::Result<ShadowRetrievalResult>;
}

/// Result of a shadow memory retrieval for one trial variant.
#[derive(Debug, Clone)]
pub struct ShadowRetrievalResult {
    pub memory_ids: Vec<String>,
    pub avg_score: f64,
    pub avg_age_days: f64,
    pub total_retrieved: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricSnapshot {
    pub correction_rate: f64,
    pub classification_accuracy: f64,
    pub avg_tokens_per_message: f64,
    pub avg_response_time_ms: f64,
    pub routing_stability: f64,
    pub memory_relevance: f64,
    pub user_satisfaction: Option<f64>,
    pub total_messages: u32,
    // Phase 2: fraction of messages where shadow retrieval overlapped with control
    pub retrieval_precision: f64,
    // Phase 2: inverse of memory-miss rate (1.0 - memory_miss_corrections / total_messages)
    pub retrieval_recall: f64,
    // Phase 2: average age (days) of retrieved memories
    pub memory_freshness: f64,
    // Phase 2: fact extraction quality (1.0 - fast_failure_rate)
    pub promotion_accuracy: f64,
    /// Phase 3: importance-weighted avg retention across active knowledge atoms.
    /// Long-term metric — changes over weeks, not per-trial.
    pub knowledge_retention_score: f64,
}
