use std::sync::Arc;

use chrono::Utc;
use common::TrialParams;
use config::AutoTunerConfig;
use storage::TrialRepo;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::evaluator::{parameter_distance, ConstraintEvaluator};
use crate::metrics::aggregate_to_result;
use crate::traits::MetricSource;
use crate::trial::TrialResult;

/// Outcome of a single nightly evaluation cycle.
#[derive(Debug)]
pub struct CycleResult {
    /// If a trial was promoted, holds `(trial_id, result, params)`.
    pub promotion: Option<(Uuid, TrialResult, TrialParams)>,
    /// `true` when the champion's live metrics have regressed.
    pub regression: bool,
    /// Number of trials that were evaluated (had enough messages).
    pub completed_count: usize,
    /// Trials that failed one or more promotion constraints, with descriptions.
    pub failed_constraints: Vec<(Uuid, Vec<String>)>,
}

/// Orchestrates the nightly evaluation-and-promotion cycle.
///
/// Responsibilities:
/// 1. Evaluate active trials that have accumulated enough messages.
/// 2. Apply promotion constraints and pick the best candidate.
/// 3. Detect regression of the current champion's live metrics.
///
/// The `NightlyCycle` does **not** call the LLM to generate new variants --
/// that is the responsibility of the L5 orchestrator which calls
/// [`NightlyCycle`] for evaluation/promotion and the generator separately.
pub struct NightlyCycle {
    config: AutoTunerConfig,
    evaluator: ConstraintEvaluator,
    repo: TrialRepo,
    metric_source: Arc<dyn MetricSource>,
}

impl NightlyCycle {
    pub fn new(
        config: AutoTunerConfig,
        repo: TrialRepo,
        metric_source: Arc<dyn MetricSource>,
    ) -> Self {
        let evaluator = ConstraintEvaluator::from_config(&config);
        Self {
            config,
            evaluator,
            repo,
            metric_source,
        }
    }

    /// Run the full evaluation-and-promotion cycle.
    ///
    /// 1. Fetch all active trials from the repo.
    /// 2. For each, collect metrics from the last 24 hours.
    /// 3. Skip trials that haven't reached `min_messages_for_promotion`.
    /// 4. Evaluate each against the `champion` baseline.
    /// 5. Among trials passing all constraints, pick the best by correction
    ///    improvement plus a small diversity bonus.
    /// 6. Return `CycleResult` with promotion candidate, regression flag, and
    ///    per-trial failure details.
    pub async fn run_evaluation_and_promotion(
        &self,
        champion: &crate::trial::Champion,
    ) -> common::Result<CycleResult> {
        let since = Utc::now() - chrono::Duration::hours(24);
        let active_trials = self.repo.get_active_trials().await.map_err(|e| {
            common::KlyntbotError::Storage(format!("failed to fetch active trials: {e}"))
        })?;

        info!(
            trial_count = active_trials.len(),
            "evaluating active trials"
        );

        let baseline = &champion.baseline_metrics;
        let mut candidates: Vec<(Uuid, TrialResult, TrialParams, f64)> = Vec::new();
        let mut failed_constraints: Vec<(Uuid, Vec<String>)> = Vec::new();
        let mut completed_count: usize = 0;

        for row in &active_trials {
            let trial_id = Uuid::parse_str(&row.id).unwrap_or_else(|_| Uuid::nil());

            // Collect metrics for this specific trial over the last 24h.
            let snapshot = self
                .metric_source
                .collect_metrics(since, Some(trial_id))
                .await?;

            let result = aggregate_to_result(trial_id, &[snapshot]);

            // Skip trials without enough data.
            if result.messages_scored < self.config.min_messages_for_promotion {
                debug!(
                    trial_id = %trial_id,
                    messages = result.messages_scored,
                    min = self.config.min_messages_for_promotion,
                    "skipping trial — insufficient messages"
                );
                continue;
            }

            completed_count += 1;

            // Evaluate promotion constraints.
            let verdict = self.evaluator.evaluate(&result, baseline);

            if verdict.passes_all() {
                let params: TrialParams = serde_json::from_str(&row.params).unwrap_or_default();
                let dist = parameter_distance(&params, &champion.params);
                candidates.push((trial_id, result, params, dist));
            } else {
                let descriptions: Vec<String> = verdict
                    .failures
                    .iter()
                    .map(|f| f.description.clone())
                    .collect();
                warn!(
                    trial_id = %trial_id,
                    failures = ?descriptions,
                    "trial failed promotion constraints"
                );
                failed_constraints.push((trial_id, descriptions));
            }
        }

        // Among passing candidates, pick the best by:
        //   score = correction_improvement + 0.1 * normalized_diversity
        let promotion = if candidates.is_empty() {
            None
        } else {
            let max_dist = candidates
                .iter()
                .map(|(_, _, _, d)| *d)
                .fold(0.0_f64, f64::max);

            candidates
                .into_iter()
                .max_by(|a, b| {
                    let score_a =
                        correction_improvement(&a.1, baseline) + diversity_bonus(a.3, max_dist);
                    let score_b =
                        correction_improvement(&b.1, baseline) + diversity_bonus(b.3, max_dist);
                    score_a
                        .partial_cmp(&score_b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(id, result, params, _)| (id, result, params))
        };

        // Check for champion regression.
        let regression = self.check_regression(champion).await?;

        Ok(CycleResult {
            promotion,
            regression,
            completed_count,
            failed_constraints,
        })
    }

    /// Check whether the champion's live metrics have regressed.
    ///
    /// Collects overall (non-trial-specific) metrics for the last 24 hours
    /// and compares `correction_rate` against the champion baseline.
    /// Returns `true` if regression is detected (i.e. correction rate has
    /// worsened by more than a small tolerance).
    pub async fn check_regression(
        &self,
        champion: &crate::trial::Champion,
    ) -> common::Result<bool> {
        let since = Utc::now() - chrono::Duration::hours(24);
        let current = self.metric_source.collect_metrics(since, None).await?;

        if current.total_messages == 0 {
            return Ok(false);
        }

        let baseline_cr = champion.baseline_metrics.correction_rate;
        if baseline_cr <= 0.0 {
            return Ok(false);
        }

        // Regression: current correction_rate is worse (higher) by more than
        // the min improvement threshold.  We reuse the same threshold as a
        // symmetrical regression detector.
        let worsening = (current.correction_rate - baseline_cr) / baseline_cr;
        let regressed = worsening > self.config.min_correction_improvement;

        if regressed {
            warn!(
                baseline_cr,
                current_cr = current.correction_rate,
                worsening_pct = worsening * 100.0,
                "champion regression detected"
            );
        }

        Ok(regressed)
    }
}

/// Fractional improvement in correction_rate (lower is better).
/// Returns a positive value when the trial improves on the baseline.
fn correction_improvement(trial: &TrialResult, baseline: &TrialResult) -> f64 {
    if baseline.correction_rate <= 0.0 {
        return 0.0;
    }
    (baseline.correction_rate - trial.correction_rate) / baseline.correction_rate
}

/// Small bonus for parameter-space diversity, scaled to [0, 0.1].
fn diversity_bonus(distance: f64, max_distance: f64) -> f64 {
    if max_distance <= 0.0 {
        return 0.0;
    }
    0.1 * (distance / max_distance)
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use uuid::Uuid;

    use crate::traits::{MetricSnapshot, MetricSource};
    use crate::trial::{Champion, TrialResult};

    use super::*;

    // ── Helpers ─────────────────────────────────────────────────────────

    /// A mock `MetricSource` that returns pre-configured snapshots.
    struct MockMetricSource {
        /// Snapshot returned for trial-specific queries (keyed by trial_id).
        trial_snapshots: std::collections::HashMap<Uuid, MetricSnapshot>,
        /// Snapshot returned for overall (non-trial) queries.
        overall_snapshot: MetricSnapshot,
    }

    #[async_trait]
    impl MetricSource for MockMetricSource {
        async fn collect_metrics(
            &self,
            _since: DateTime<Utc>,
            trial_id: Option<Uuid>,
        ) -> common::Result<MetricSnapshot> {
            match trial_id {
                Some(id) => Ok(self.trial_snapshots.get(&id).cloned().unwrap_or_default()),
                None => Ok(self.overall_snapshot.clone()),
            }
        }
    }

    fn default_champion() -> Champion {
        Champion {
            trial_id: None,
            params: TrialParams::default(),
            promoted_at: Utc::now(),
            baseline_metrics: TrialResult {
                trial_id: Uuid::nil(),
                messages_scored: 200,
                correction_rate: 0.20,
                classification_accuracy: 0.85,
                avg_tokens_per_message: 500.0,
                avg_response_time_ms: 800.0,
                routing_stability: 0.90,
                memory_relevance: 0.80,
                user_satisfaction: None,
            },
            reason_for_promotion: "baseline".into(),
            impact_summary: "baseline".into(),
            consecutive_regression_days: 0,
        }
    }

    async fn setup_repo() -> TrialRepo {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("PRAGMA foreign_keys=ON;")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(storage::repos::trial_repo::MIGRATION_SQL)
            .execute(&pool)
            .await
            .unwrap();
        TrialRepo::new(pool)
    }

    async fn insert_active_trial(
        repo: &TrialRepo,
        trial_id: Uuid,
        experiment_id: &str,
        params: &TrialParams,
    ) {
        use storage::rows::trial::{ExperimentRow, TrialRow};

        // Create experiment if needed (ignore duplicate errors).
        let _ = repo
            .create_experiment(&ExperimentRow {
                id: experiment_id.to_string(),
                hypothesis: "test".into(),
                trend_analysis: "test".into(),
                recommendation_for_next: "test".into(),
                created_at: Utc::now().to_rfc3339(),
            })
            .await;

        repo.create_trial(&TrialRow {
            id: trial_id.to_string(),
            experiment_id: experiment_id.to_string(),
            params: serde_json::to_string(params).unwrap(),
            generation_reasoning: "test trial".into(),
            status: "active".into(),
            created_at: Utc::now().to_rfc3339(),
            completed_at: None,
            result: None,
        })
        .await
        .unwrap();
    }

    // ── Tests ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn promotes_best_candidate() {
        let repo = setup_repo().await;
        let champion = default_champion();

        let trial_a = Uuid::new_v4();
        let trial_b = Uuid::new_v4();

        let params_a = TrialParams {
            skill_keyword_weight: Some(0.55),
            ..Default::default()
        };
        let params_b = TrialParams {
            skill_keyword_weight: Some(0.30),
            ..Default::default()
        };

        insert_active_trial(&repo, trial_a, "exp-1", &params_a).await;
        insert_active_trial(&repo, trial_b, "exp-1", &params_b).await;

        // trial_a: correction_rate 0.18 (10% improvement on 0.20 baseline)
        // trial_b: correction_rate 0.15 (25% improvement — much better)
        let mut trial_snapshots = std::collections::HashMap::new();
        trial_snapshots.insert(
            trial_a,
            MetricSnapshot {
                correction_rate: 0.18,
                classification_accuracy: 0.86,
                avg_tokens_per_message: 510.0,
                avg_response_time_ms: 820.0,
                routing_stability: 0.89,
                memory_relevance: 0.79,
                user_satisfaction: None,
                total_messages: 100,
            },
        );
        trial_snapshots.insert(
            trial_b,
            MetricSnapshot {
                correction_rate: 0.15,
                classification_accuracy: 0.87,
                avg_tokens_per_message: 490.0,
                avg_response_time_ms: 790.0,
                routing_stability: 0.88,
                memory_relevance: 0.78,
                user_satisfaction: None,
                total_messages: 100,
            },
        );

        let source = Arc::new(MockMetricSource {
            trial_snapshots,
            overall_snapshot: MetricSnapshot::default(),
        });

        let config = AutoTunerConfig {
            min_messages_for_promotion: 50,
            ..Default::default()
        };
        let cycle = NightlyCycle::new(config, repo, source);
        let result = cycle.run_evaluation_and_promotion(&champion).await.unwrap();

        assert!(result.promotion.is_some(), "should promote a trial");
        let (promoted_id, _, _) = result.promotion.unwrap();
        // trial_b has better correction improvement, should win
        assert_eq!(promoted_id, trial_b);
        assert_eq!(result.completed_count, 2);
    }

    #[tokio::test]
    async fn skips_trials_with_insufficient_messages() {
        let repo = setup_repo().await;
        let champion = default_champion();

        let trial_id = Uuid::new_v4();
        insert_active_trial(&repo, trial_id, "exp-2", &TrialParams::default()).await;

        let mut trial_snapshots = std::collections::HashMap::new();
        trial_snapshots.insert(
            trial_id,
            MetricSnapshot {
                total_messages: 10, // below the 50 threshold
                correction_rate: 0.10,
                ..Default::default()
            },
        );

        let source = Arc::new(MockMetricSource {
            trial_snapshots,
            overall_snapshot: MetricSnapshot::default(),
        });

        let config = AutoTunerConfig {
            min_messages_for_promotion: 50,
            ..Default::default()
        };
        let cycle = NightlyCycle::new(config, repo, source);
        let result = cycle.run_evaluation_and_promotion(&champion).await.unwrap();

        assert!(result.promotion.is_none());
        assert_eq!(result.completed_count, 0);
    }

    #[tokio::test]
    async fn records_constraint_failures() {
        let repo = setup_repo().await;
        let champion = default_champion();

        let trial_id = Uuid::new_v4();
        insert_active_trial(&repo, trial_id, "exp-3", &TrialParams::default()).await;

        // Trial that regresses token cost beyond the 8% threshold
        let mut trial_snapshots = std::collections::HashMap::new();
        trial_snapshots.insert(
            trial_id,
            MetricSnapshot {
                correction_rate: 0.18,
                classification_accuracy: 0.86,
                avg_tokens_per_message: 600.0, // +20% — fails constraint
                avg_response_time_ms: 810.0,
                routing_stability: 0.89,
                memory_relevance: 0.79,
                user_satisfaction: None,
                total_messages: 100,
            },
        );

        let source = Arc::new(MockMetricSource {
            trial_snapshots,
            overall_snapshot: MetricSnapshot::default(),
        });

        let config = AutoTunerConfig {
            min_messages_for_promotion: 50,
            ..Default::default()
        };
        let cycle = NightlyCycle::new(config, repo, source);
        let result = cycle.run_evaluation_and_promotion(&champion).await.unwrap();

        assert!(result.promotion.is_none());
        assert_eq!(result.failed_constraints.len(), 1);
        let (failed_id, descriptions) = &result.failed_constraints[0];
        assert_eq!(*failed_id, trial_id);
        assert!(
            descriptions.iter().any(|d| d.contains("token cost")),
            "should report token cost failure, got: {descriptions:?}"
        );
    }

    #[tokio::test]
    async fn detects_regression() {
        let repo = setup_repo().await;
        let champion = default_champion(); // baseline correction_rate = 0.20

        // Current overall metrics have regressed (correction_rate much higher)
        let source = Arc::new(MockMetricSource {
            trial_snapshots: std::collections::HashMap::new(),
            overall_snapshot: MetricSnapshot {
                correction_rate: 0.30, // 50% worse than 0.20 baseline
                total_messages: 100,
                ..Default::default()
            },
        });

        let config = AutoTunerConfig::default();
        let cycle = NightlyCycle::new(config, repo, source);
        let regressed = cycle.check_regression(&champion).await.unwrap();

        assert!(
            regressed,
            "should detect regression when correction_rate worsens significantly"
        );
    }

    #[tokio::test]
    async fn no_regression_when_stable() {
        let repo = setup_repo().await;
        let champion = default_champion(); // baseline correction_rate = 0.20

        // Current overall metrics are similar to baseline
        let source = Arc::new(MockMetricSource {
            trial_snapshots: std::collections::HashMap::new(),
            overall_snapshot: MetricSnapshot {
                correction_rate: 0.20,
                total_messages: 100,
                ..Default::default()
            },
        });

        let config = AutoTunerConfig::default();
        let cycle = NightlyCycle::new(config, repo, source);
        let regressed = cycle.check_regression(&champion).await.unwrap();

        assert!(
            !regressed,
            "should not detect regression when metrics are stable"
        );
    }

    #[tokio::test]
    async fn no_trials_returns_empty_result() {
        let repo = setup_repo().await;
        let champion = default_champion();

        let source = Arc::new(MockMetricSource {
            trial_snapshots: std::collections::HashMap::new(),
            overall_snapshot: MetricSnapshot {
                correction_rate: 0.19,
                total_messages: 100,
                ..Default::default()
            },
        });

        let config = AutoTunerConfig::default();
        let cycle = NightlyCycle::new(config, repo, source);
        let result = cycle.run_evaluation_and_promotion(&champion).await.unwrap();

        assert!(result.promotion.is_none());
        assert_eq!(result.completed_count, 0);
        assert!(result.failed_constraints.is_empty());
    }

    #[test]
    fn correction_improvement_helper() {
        let baseline = TrialResult {
            correction_rate: 0.20,
            ..Default::default()
        };
        let trial = TrialResult {
            correction_rate: 0.15,
            ..Default::default()
        };
        let improvement = super::correction_improvement(&trial, &baseline);
        assert!((improvement - 0.25).abs() < 1e-9); // (0.20 - 0.15) / 0.20 = 0.25
    }

    #[test]
    fn diversity_bonus_scales_correctly() {
        assert!((super::diversity_bonus(5.0, 10.0) - 0.05).abs() < 1e-9);
        assert!((super::diversity_bonus(10.0, 10.0) - 0.1).abs() < 1e-9);
        assert!((super::diversity_bonus(0.0, 10.0) - 0.0).abs() < 1e-9);
        assert!((super::diversity_bonus(5.0, 0.0) - 0.0).abs() < 1e-9);
    }
}
