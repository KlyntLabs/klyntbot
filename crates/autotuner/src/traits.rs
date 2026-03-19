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
        since: chrono::DateTime<chrono::Utc>,
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
}
