//! Implements [`MetricSource`] by reading from [`StrategyRepo`].

use async_trait::async_trait;
use autotuner::{MetricSnapshot, MetricSource};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Collects metrics from the strategy_records table for autotuner evaluation.
pub struct AgentMetricCollector {
    strategy_repo: storage::StrategyRepo,
}

impl AgentMetricCollector {
    pub fn new(strategy_repo: storage::StrategyRepo) -> Self {
        Self { strategy_repo }
    }
}

#[async_trait]
impl MetricSource for AgentMetricCollector {
    async fn collect_metrics(
        &self,
        since: DateTime<Utc>,
        _trial_id: Option<Uuid>,
    ) -> common::Result<MetricSnapshot> {
        let stats = self
            .strategy_repo
            .get_stats_since(since)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        Ok(MetricSnapshot {
            correction_rate: 0.0, // TODO: count UserCorrectedAI events
            classification_accuracy: stats.accuracy,
            avg_tokens_per_message: 0.0, // TODO: from usage_records
            avg_response_time_ms: stats.avg_response_time_ms as f64,
            routing_stability: 1.0, // TODO: compute from shadow_log
            memory_relevance: 1.0,  // Placeholder until Phase 2
            user_satisfaction: stats.avg_satisfaction,
            total_messages: stats.total_records as u32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn collects_from_empty_repo() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repo = storage::StrategyRepo::new(pool.inner().clone());
        let collector = AgentMetricCollector::new(repo);

        let since = Utc::now() - chrono::Duration::days(1);
        let snapshot = collector.collect_metrics(since, None).await.unwrap();

        assert_eq!(snapshot.total_messages, 0);
        assert_eq!(snapshot.classification_accuracy, 0.0);
        assert!(snapshot.user_satisfaction.is_none());
    }
}
