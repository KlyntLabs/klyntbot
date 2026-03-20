//! Implements [`MetricSource`] by reading from real repositories:
//! [`StrategyRepo`], [`EventLogRepo`], [`UsageRepo`], and [`TrialRepo`].

use async_trait::async_trait;
use autotuner::{MetricSnapshot, MetricSource};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Collects metrics from strategy records, domain events, usage records,
/// and shadow-log trial data for autotuner evaluation.
pub struct AgentMetricCollector {
    strategy_repo: storage::StrategyRepo,
    event_log_repo: cognitive::EventLogRepo,
    usage_repo: storage::UsageRepo,
    trial_repo: storage::TrialRepo,
}

impl AgentMetricCollector {
    pub fn new(
        strategy_repo: storage::StrategyRepo,
        event_log_repo: cognitive::EventLogRepo,
        usage_repo: storage::UsageRepo,
        trial_repo: storage::TrialRepo,
    ) -> Self {
        Self {
            strategy_repo,
            event_log_repo,
            usage_repo,
            trial_repo,
        }
    }
}

#[async_trait]
impl MetricSource for AgentMetricCollector {
    async fn collect_metrics(
        &self,
        since: DateTime<Utc>,
        trial_id: Option<Uuid>,
    ) -> common::Result<MetricSnapshot> {
        let trial_id_str = trial_id.as_ref().map(|u| u.to_string());

        let (stats, correction_count, token_info, routing_stability) = tokio::join!(
            self.strategy_repo.get_stats_since(since),
            self.event_log_repo
                .count_by_event_type("UserCorrectedAI", since),
            async {
                let tokens = self.usage_repo.total_tokens_since(since).await.unwrap_or(0);
                let (reqs, _) = self
                    .usage_repo
                    .totals_since(since)
                    .await
                    .unwrap_or((0, 0.0));
                (tokens, reqs)
            },
            self.trial_repo
                .shadow_log_agreement_rate(trial_id_str.as_deref(), since),
        );

        let stats = stats.map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        let correction_count = correction_count.unwrap_or(0);
        let (total_tokens, total_requests) = token_info;
        let routing_stability = routing_stability.unwrap_or(1.0);

        let total = stats.total_records.max(1);
        let correction_rate = (correction_count as f64 / total as f64).min(1.0);
        let avg_tokens_per_message = total_tokens as f64 / total_requests.max(1) as f64;

        // memory_relevance — Phase 2 placeholder
        let memory_relevance = 1.0;

        Ok(MetricSnapshot {
            correction_rate,
            classification_accuracy: stats.accuracy,
            avg_tokens_per_message,
            avg_response_time_ms: stats.avg_response_time_ms as f64,
            routing_stability,
            memory_relevance,
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
        let inner = pool.inner().clone();
        let strategy_repo = storage::StrategyRepo::new(inner.clone());
        let event_log_repo = cognitive::EventLogRepo::new(inner.clone());
        let usage_repo = storage::UsageRepo::new(inner.clone());
        let trial_repo = storage::TrialRepo::new(inner.clone());
        trial_repo.migrate().await.unwrap();
        let collector =
            AgentMetricCollector::new(strategy_repo, event_log_repo, usage_repo, trial_repo);

        let since = Utc::now() - chrono::Duration::days(1);
        let snapshot = collector.collect_metrics(since, None).await.unwrap();

        assert_eq!(snapshot.total_messages, 0);
        assert_eq!(snapshot.classification_accuracy, 0.0);
        assert!(snapshot.user_satisfaction.is_none());
        // Empty repos should yield safe defaults
        assert_eq!(snapshot.correction_rate, 0.0);
        assert_eq!(snapshot.avg_tokens_per_message, 0.0);
        assert_eq!(snapshot.routing_stability, 1.0);
        assert_eq!(snapshot.memory_relevance, 1.0);
    }
}
