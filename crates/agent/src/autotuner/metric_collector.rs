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
    fact_repo: cognitive::SemanticFactRepo,
    review_stats: cognitive::ReviewStatsRepo,
    feedback_repo: Option<storage::RetrievalFeedbackRepo>,
}

impl AgentMetricCollector {
    pub fn new(
        strategy_repo: storage::StrategyRepo,
        event_log_repo: cognitive::EventLogRepo,
        usage_repo: storage::UsageRepo,
        trial_repo: storage::TrialRepo,
        fact_repo: cognitive::SemanticFactRepo,
    ) -> Self {
        let review_stats = cognitive::ReviewStatsRepo::new(fact_repo.pool().clone());
        Self {
            strategy_repo,
            event_log_repo,
            usage_repo,
            trial_repo,
            fact_repo,
            review_stats,
            feedback_repo: None,
        }
    }

    pub fn with_feedback_repo(mut self, repo: storage::RetrievalFeedbackRepo) -> Self {
        self.feedback_repo = Some(repo);
        self
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

        let (
            stats,
            correction_count,
            memory_miss_count,
            token_info,
            routing_stability,
            memory_relevance_opt,
            per_trial_correction,
            phase2_metrics,
            fact_health_result,
            knowledge_retention_result,
            rewrite_trigger_result,
            rewrite_engagement_result,
        ) = tokio::join!(
            self.strategy_repo.get_stats_since(since),
            self.event_log_repo
                .count_by_event_type("UserCorrectedAI", since),
            self.event_log_repo.count_by_event_type_and_data(
                "UserCorrectedAI",
                "memory_miss",
                since
            ),
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
            self.strategy_repo.memory_relevance_since(since),
            async {
                if let Some(tid) = trial_id_str.as_deref() {
                    self.trial_repo
                        .correction_rate_for_trial(tid, since)
                        .await
                        .ok()
                } else {
                    None
                }
            },
            // Phase 2 metrics from shadow retrieval log
            async {
                if let Some(tid) = trial_id_str.as_deref() {
                    let precision = self
                        .trial_repo
                        .retrieval_precision_for_trial(tid, since)
                        .await
                        .unwrap_or(0.0);
                    let freshness = self
                        .trial_repo
                        .avg_memory_freshness_for_trial(tid, since)
                        .await
                        .unwrap_or(0.0);
                    (precision, freshness)
                } else {
                    (0.0, 0.0)
                }
            },
            // Fact health (Knowledge Trust) for promotion_accuracy
            self.fact_repo.fact_health_by_domain(90),
            // Phase 3: importance-weighted avg retention across active knowledge atoms
            self.review_stats.knowledge_retention_score(),
            // Phase 3: rewrite metrics
            self.strategy_repo.rewrite_trigger_rate_since(since),
            self.strategy_repo.rewrite_engagement_rate_since(since),
        );
        let (shadow_precision, memory_freshness) = phase2_metrics;

        // Blend shadow-log precision with actual retrieval feedback when available
        let retrieval_precision = if let Some(ref repo) = self.feedback_repo {
            let feedback_precision: f64 = repo.avg_precision_since(7).await.unwrap_or(0.0);
            let feedback_count: i64 = repo.count_since(7).await.unwrap_or(0);
            if feedback_count > 0 && shadow_precision > 0.0 {
                // Weighted average: feedback has more ground truth
                feedback_precision * 0.7 + shadow_precision * 0.3
            } else if feedback_count > 0 {
                feedback_precision
            } else {
                shadow_precision
            }
        } else {
            shadow_precision
        };

        let promotion_accuracy = match fact_health_result {
            Ok(ref domains) => cognitive::DomainHealthRow::average_health(domains),
            _ => 1.0,
        };

        let knowledge_retention_score = knowledge_retention_result.unwrap_or(1.0);
        let rewrite_trigger_rate = rewrite_trigger_result.unwrap_or(0.0);
        let rewrite_engagement_rate = rewrite_engagement_result.unwrap_or(0.0);

        let stats = stats.map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        let correction_count = correction_count.unwrap_or(0);
        let memory_miss_count = memory_miss_count.unwrap_or(0);
        let (total_tokens, total_requests) = token_info;
        let routing_stability = routing_stability.unwrap_or(1.0);

        let correction_rate = if let Some((total, corrected)) = per_trial_correction {
            if total == 0 {
                0.0
            } else {
                (corrected as f64 / total as f64).min(1.0)
            }
        } else {
            // System-wide fallback
            let total = stats.total_records.max(1);
            (correction_count as f64 / total as f64).min(1.0)
        };
        let avg_tokens_per_message = total_tokens as f64 / total_requests.max(1) as f64;

        // retrieval_recall: 1.0 - (memory_miss_corrections / total_messages)
        // Higher is better — 1.0 means no memory misses.
        let total = stats.total_records.max(1) as f64;
        let retrieval_recall = (1.0 - memory_miss_count as f64 / total).max(0.0);

        // memory_relevance: fraction of messages where memories were retrieved.
        // Falls back to 1.0 when no records have the field populated yet
        // (e.g. historical data from before this column was added).
        let memory_relevance = match memory_relevance_opt {
            Ok(Some(val)) => val,
            _ => 1.0,
        };

        Ok(MetricSnapshot {
            correction_rate,
            classification_accuracy: stats.accuracy,
            avg_tokens_per_message,
            avg_response_time_ms: stats.avg_response_time_ms as f64,
            routing_stability,
            memory_relevance,
            user_satisfaction: stats.avg_satisfaction,
            total_messages: stats.total_records as u32,
            retrieval_precision,
            retrieval_recall,
            memory_freshness,
            promotion_accuracy,
            knowledge_retention_score,
            rewrite_trigger_rate,
            rewrite_engagement_rate,
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
        let collector = AgentMetricCollector::new(
            strategy_repo,
            event_log_repo,
            usage_repo,
            trial_repo,
            cognitive::SemanticFactRepo::new(inner.clone()),
        );

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

    #[tokio::test]
    async fn collects_real_metric_values() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();

        // Run cognitive migrations so domain_event_log table exists
        let cog_migrations = cognitive::cognitive_migrations();
        storage::StoragePool::run_feature_migrations(&inner, &cog_migrations)
            .await
            .unwrap();

        let strategy_repo = storage::StrategyRepo::new(inner.clone());
        let event_log_repo = cognitive::EventLogRepo::new(inner.clone());
        let usage_repo = storage::UsageRepo::new(inner.clone());
        let trial_repo = storage::TrialRepo::new(inner.clone());
        trial_repo.migrate().await.unwrap();

        let since = Utc::now() - chrono::Duration::hours(1);

        // 1. Insert strategy records so get_stats_since returns total_records > 0.
        //    Two records: one with memories retrieved (3), one without (0).
        //    Expected memory_relevance = 1/2 = 0.5.
        let strategy_row = storage::rows::learning::StrategyRecordRow {
            id: uuid::Uuid::new_v4(),
            timestamp: Utc::now(),
            request_id: "req-metric-test".to_string(),
            predicted_strategy: "DirectResponse".to_string(),
            actual_strategy: "DirectResponse".to_string(),
            escalation_count: 0,
            iterations_used: 1,
            max_iterations: 1,
            success: true,
            user_satisfaction: None,
            response_time_ms: 500,
            chat_id: Some("test-chat".to_string()),
            tool_name: None,
            tool_success: None,
            tool_duration_ms: None,
            complexity_signals: serde_json::Value::Null,
            execution_mode: None,
            retrieved_memory_count: Some(3),
            rewrite_triggered: 0,
            rewrite_source: None,
            budget_exhausted: false,
            turns_used: 0,
            loop_detected: false,
            loop_tools: None,
            context_fill_pct: None,
        };
        strategy_repo.create(&strategy_row).await.unwrap();

        let strategy_row_no_mem = storage::rows::learning::StrategyRecordRow {
            id: uuid::Uuid::new_v4(),
            timestamp: Utc::now(),
            request_id: "req-metric-test-2".to_string(),
            predicted_strategy: "DirectResponse".to_string(),
            actual_strategy: "DirectResponse".to_string(),
            escalation_count: 0,
            iterations_used: 1,
            max_iterations: 1,
            success: true,
            user_satisfaction: None,
            response_time_ms: 400,
            chat_id: Some("test-chat-2".to_string()),
            tool_name: None,
            tool_success: None,
            tool_duration_ms: None,
            complexity_signals: serde_json::Value::Null,
            execution_mode: None,
            retrieved_memory_count: Some(0),
            rewrite_triggered: 0,
            rewrite_source: None,
            budget_exhausted: false,
            turns_used: 0,
            loop_detected: false,
            loop_tools: None,
            context_fill_pct: None,
        };
        strategy_repo.create(&strategy_row_no_mem).await.unwrap();

        // 2. Insert a domain event with event_type = "UserCorrectedAI"
        event_log_repo
            .insert_domain_event(
                "evt-correction-1",
                "UserCorrectedAI",
                "general",
                "extract",
                r#"{"msg":"no that's wrong"}"#,
                &Utc::now().to_rfc3339(),
            )
            .await
            .unwrap();

        // 3. Insert a usage record so avg_tokens_per_message > 0
        let usage_row = storage::rows::usage::UsageRecordRow {
            id: uuid::Uuid::new_v4(),
            timestamp: Utc::now(),
            request_id: "req-usage-test".to_string(),
            model: "gpt-4o".to_string(),
            provider: "openai".to_string(),
            prompt_tokens: 200,
            completion_tokens: 100,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            estimated_cost_usd: 0.01,
            channel: "telegram".to_string(),
            strategy: "DirectResponse".to_string(),
        };
        usage_repo.create(&usage_row).await.unwrap();

        // 4. Insert shadow log rows with real ground truth (not "pending")
        let exp = storage::rows::trial::ExperimentRow {
            id: "exp-metric".to_string(),
            hypothesis: "test".to_string(),
            trend_analysis: "test".to_string(),
            recommendation_for_next: "test".to_string(),
            created_at: Utc::now().to_rfc3339(),
        };
        trial_repo.create_experiment(&exp).await.unwrap();

        let trial = storage::rows::trial::TrialRow {
            id: "trial-metric".to_string(),
            experiment_id: "exp-metric".to_string(),
            params: serde_json::to_string(&common::TrialParams::default()).unwrap(),
            generation_reasoning: "test".to_string(),
            status: "active".to_string(),
            created_at: Utc::now().to_rfc3339(),
            completed_at: None,
            result: None,
        };
        trial_repo.create_trial(&trial).await.unwrap();

        // Shadow log entry where predicted matches ground truth
        trial_repo
            .insert_shadow_log(
                "trial-metric",
                &Utc::now().to_rfc3339(),
                "test-chat",
                "",
                "general",
                "direct",
                0.9,
                1,
                "general",
                "direct",
            )
            .await
            .unwrap();

        // Shadow log entry where predicted differs from ground truth
        trial_repo
            .insert_shadow_log(
                "trial-metric",
                &Utc::now().to_rfc3339(),
                "test-chat-2",
                "",
                "general",
                "direct",
                0.7,
                1,
                "general",
                "reactive",
            )
            .await
            .unwrap();

        let collector = AgentMetricCollector::new(
            strategy_repo,
            event_log_repo,
            usage_repo,
            trial_repo,
            cognitive::SemanticFactRepo::new(inner.clone()),
        );

        // ── System-wide fallback (trial_id = None) ───────────────────────
        let snapshot_system = collector.collect_metrics(since, None).await.unwrap();

        // correction_rate = 1 correction / 2 strategy records = 0.5
        assert!(
            snapshot_system.correction_rate > 0.0,
            "Expected system-wide correction_rate > 0.0, got {}",
            snapshot_system.correction_rate
        );

        // avg_tokens_per_message = 300 tokens / 1 request = 300.0
        assert!(
            snapshot_system.avg_tokens_per_message > 0.0,
            "Expected avg_tokens_per_message > 0.0, got {}",
            snapshot_system.avg_tokens_per_message
        );

        // routing_stability: no trial_id → all shadow log rows (50% agreement)
        assert!(
            snapshot_system.routing_stability >= 0.0 && snapshot_system.routing_stability <= 1.0,
            "Expected routing_stability in [0,1], got {}",
            snapshot_system.routing_stability
        );

        // memory_relevance = 1 record with memories / 2 total records = 0.5
        assert!(
            (snapshot_system.memory_relevance - 0.5).abs() < f64::EPSILON,
            "Expected memory_relevance = 0.5, got {}",
            snapshot_system.memory_relevance
        );

        // total_messages should be 2 (from the strategy records)
        assert_eq!(snapshot_system.total_messages, 2);

        // ── Per-trial path (unmatched UUID → 0 shadow rows → 0.0) ────────
        let snapshot_trial = collector
            .collect_metrics(
                since,
                Some(Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap()),
            )
            .await
            .unwrap();

        assert_eq!(
            snapshot_trial.correction_rate, 0.0,
            "Expected per-trial correction_rate = 0.0 for unmatched UUID, got {}",
            snapshot_trial.correction_rate
        );
    }
}
