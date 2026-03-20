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
    /// All evaluated trials with their aggregated results (for observability logging).
    pub evaluated_trials: Vec<(Uuid, TrialResult)>,
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
        let mut evaluated_trials: Vec<(Uuid, TrialResult)> = Vec::new();
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
            evaluated_trials.push((trial_id, result.clone()));

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
            evaluated_trials,
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

/// Returns the names of `TrialParams` fields that differ between two param sets.
pub fn affected_param_names(old: &TrialParams, new: &TrialParams) -> Vec<String> {
    let mut names = Vec::new();
    macro_rules! check_field {
        ($field:ident) => {
            if old.$field != new.$field {
                names.push(stringify!($field).to_string());
            }
        };
    }
    check_field!(skill_keyword_weight);
    check_field!(skill_semantic_weight);
    check_field!(skill_activation_threshold);
    check_field!(heuristic_confidence_threshold);
    check_field!(llm_classifier_timeout_ms);
    check_field!(relevance_weight_semantic);
    check_field!(relevance_weight_retrievability);
    check_field!(relevance_weight_situation);
    check_field!(fsrs_desired_retention);
    check_field!(accumulate_promote_threshold);
    check_field!(accumulate_min_days);
    check_field!(vector_top_k);
    check_field!(min_similarity);
    check_field!(relevance_weight_importance);
    check_field!(relevance_weight_frequency);
    check_field!(relevance_weight_temporal);
    names
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
                ..Default::default()
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
                ..Default::default()
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

    #[test]
    fn affected_param_names_finds_differences() {
        let old = TrialParams {
            heuristic_confidence_threshold: Some(0.7),
            skill_keyword_weight: Some(0.5),
            ..Default::default()
        };
        let new = TrialParams {
            heuristic_confidence_threshold: Some(0.8),
            skill_keyword_weight: Some(0.5),
            skill_semantic_weight: Some(0.3),
            ..Default::default()
        };
        let names = super::affected_param_names(&old, &new);
        assert!(names.contains(&"heuristic_confidence_threshold".to_string()));
        assert!(names.contains(&"skill_semantic_weight".to_string()));
        assert!(!names.contains(&"skill_keyword_weight".to_string()));
    }

    // ── Integration tests ──────────────────────────────────────────────

    /// End-to-end: seed 3 trials with varying quality.  Trial A has great
    /// metrics (passes all constraints), Trial B fails the token cost
    /// constraint, Trial C has bad correction rate.  Assert that only
    /// Trial A is promoted.
    #[tokio::test]
    async fn full_evaluation_cycle_promotes_winning_trial() {
        let repo = setup_repo().await;
        let champion = default_champion();
        // champion baseline: correction_rate=0.20, tokens=500, response_time=800,
        //                    routing_stability=0.90, memory_relevance=0.80

        let trial_a = Uuid::new_v4();
        let trial_b = Uuid::new_v4();
        let trial_c = Uuid::new_v4();

        let params_a = TrialParams {
            skill_keyword_weight: Some(0.55),
            ..Default::default()
        };
        let params_b = TrialParams {
            skill_keyword_weight: Some(0.40),
            ..Default::default()
        };
        let params_c = TrialParams {
            skill_keyword_weight: Some(0.30),
            ..Default::default()
        };

        insert_active_trial(&repo, trial_a, "exp-int-1", &params_a).await;
        insert_active_trial(&repo, trial_b, "exp-int-1", &params_b).await;
        insert_active_trial(&repo, trial_c, "exp-int-1", &params_c).await;

        let mut trial_snapshots = std::collections::HashMap::new();

        // Trial A: great metrics — passes all constraints
        //   correction improvement = (0.20 - 0.14) / 0.20 = 30% (needs >= 5%)
        //   token increase = (510 - 500) / 500 = 2% (max 8%)
        //   response time increase = (820 - 800) / 800 = 2.5% (max 15%)
        //   routing stability decrease = (0.90 - 0.88) / 0.90 = 2.2% (max 10%)
        //   memory relevance decrease = (0.80 - 0.78) / 0.80 = 2.5% (max 5%)
        trial_snapshots.insert(
            trial_a,
            MetricSnapshot {
                correction_rate: 0.14,
                classification_accuracy: 0.88,
                avg_tokens_per_message: 510.0,
                avg_response_time_ms: 820.0,
                routing_stability: 0.88,
                memory_relevance: 0.78,
                user_satisfaction: None,
                total_messages: 100,
                ..Default::default()
            },
        );

        // Trial B: ok correction but FAILS token cost constraint
        //   correction improvement = (0.20 - 0.17) / 0.20 = 15% — passes
        //   token increase = (600 - 500) / 500 = 20% — FAILS (max 8%)
        trial_snapshots.insert(
            trial_b,
            MetricSnapshot {
                correction_rate: 0.17,
                classification_accuracy: 0.86,
                avg_tokens_per_message: 600.0,
                avg_response_time_ms: 810.0,
                routing_stability: 0.89,
                memory_relevance: 0.79,
                user_satisfaction: None,
                total_messages: 100,
                ..Default::default()
            },
        );

        // Trial C: bad correction rate — fails correction improvement constraint
        //   correction improvement = (0.20 - 0.22) / 0.20 = -10% — FAILS (needs >= 5%)
        trial_snapshots.insert(
            trial_c,
            MetricSnapshot {
                correction_rate: 0.22,
                classification_accuracy: 0.84,
                avg_tokens_per_message: 490.0,
                avg_response_time_ms: 790.0,
                routing_stability: 0.91,
                memory_relevance: 0.81,
                user_satisfaction: None,
                total_messages: 100,
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

        // Trial A should be promoted (only one passing all constraints)
        assert!(result.promotion.is_some(), "should promote trial A");
        let (promoted_id, _, _) = result.promotion.unwrap();
        assert_eq!(
            promoted_id, trial_a,
            "trial A should be the promoted winner"
        );

        // All three trials had enough messages → all evaluated
        assert_eq!(result.completed_count, 3);

        // Trial B and C should appear in failed_constraints
        assert_eq!(
            result.failed_constraints.len(),
            2,
            "exactly 2 trials should fail constraints"
        );

        let failed_ids: Vec<Uuid> = result
            .failed_constraints
            .iter()
            .map(|(id, _)| *id)
            .collect();
        assert!(
            failed_ids.contains(&trial_b),
            "trial B should be in failed list"
        );
        assert!(
            failed_ids.contains(&trial_c),
            "trial C should be in failed list"
        );

        // Verify the failure reasons are specific
        let trial_b_failures = result
            .failed_constraints
            .iter()
            .find(|(id, _)| *id == trial_b)
            .map(|(_, descs)| descs)
            .unwrap();
        assert!(
            trial_b_failures.iter().any(|d| d.contains("token cost")),
            "trial B should fail on token cost, got: {trial_b_failures:?}"
        );

        let trial_c_failures = result
            .failed_constraints
            .iter()
            .find(|(id, _)| *id == trial_c)
            .map(|(_, descs)| descs)
            .unwrap();
        assert!(
            trial_c_failures
                .iter()
                .any(|d| d.contains("correction_rate")),
            "trial C should fail on correction_rate, got: {trial_c_failures:?}"
        );
    }

    /// When every trial fails at least one promotion constraint, no
    /// promotion should occur and all trials should appear in
    /// `failed_constraints`.
    #[tokio::test]
    async fn evaluation_rejects_all_when_none_pass_constraints() {
        let repo = setup_repo().await;
        let champion = default_champion();
        // champion baseline: correction_rate=0.20, tokens=500, response_time=800,
        //                    routing_stability=0.90, memory_relevance=0.80

        let trial_a = Uuid::new_v4();
        let trial_b = Uuid::new_v4();
        let trial_c = Uuid::new_v4();

        insert_active_trial(&repo, trial_a, "exp-int-2", &TrialParams::default()).await;
        insert_active_trial(&repo, trial_b, "exp-int-2", &TrialParams::default()).await;
        insert_active_trial(&repo, trial_c, "exp-int-2", &TrialParams::default()).await;

        let mut trial_snapshots = std::collections::HashMap::new();

        // Trial A: fails token cost (tokens +20%)
        //   correction improvement = (0.20 - 0.14) / 0.20 = 30% — passes
        //   token increase = (600 - 500) / 500 = 20% — FAILS (max 8%)
        trial_snapshots.insert(
            trial_a,
            MetricSnapshot {
                correction_rate: 0.14,
                classification_accuracy: 0.87,
                avg_tokens_per_message: 600.0,
                avg_response_time_ms: 810.0,
                routing_stability: 0.88,
                memory_relevance: 0.78,
                user_satisfaction: None,
                total_messages: 100,
                ..Default::default()
            },
        );

        // Trial B: fails correction improvement (only 2% improvement, needs 5%)
        //   correction improvement = (0.20 - 0.196) / 0.20 = 2% — FAILS
        trial_snapshots.insert(
            trial_b,
            MetricSnapshot {
                correction_rate: 0.196,
                classification_accuracy: 0.86,
                avg_tokens_per_message: 510.0,
                avg_response_time_ms: 810.0,
                routing_stability: 0.89,
                memory_relevance: 0.79,
                user_satisfaction: None,
                total_messages: 100,
                ..Default::default()
            },
        );

        // Trial C: fails memory relevance (drops 8%, max allowed 5%)
        //   correction improvement = (0.20 - 0.17) / 0.20 = 15% — passes
        //   memory relevance decrease = (0.80 - 0.736) / 0.80 = 8% — FAILS (max 5%)
        trial_snapshots.insert(
            trial_c,
            MetricSnapshot {
                correction_rate: 0.17,
                classification_accuracy: 0.86,
                avg_tokens_per_message: 510.0,
                avg_response_time_ms: 810.0,
                routing_stability: 0.88,
                memory_relevance: 0.736,
                user_satisfaction: None,
                total_messages: 100,
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

        // No trial should be promoted
        assert!(
            result.promotion.is_none(),
            "no trial should be promoted when all fail constraints"
        );

        // All 3 were evaluated
        assert_eq!(result.completed_count, 3);

        // All 3 should appear in failed_constraints
        assert_eq!(
            result.failed_constraints.len(),
            3,
            "all 3 trials should have constraint failures"
        );

        let failed_ids: Vec<Uuid> = result
            .failed_constraints
            .iter()
            .map(|(id, _)| *id)
            .collect();
        assert!(
            failed_ids.contains(&trial_a),
            "trial A should be in failed list"
        );
        assert!(
            failed_ids.contains(&trial_b),
            "trial B should be in failed list"
        );
        assert!(
            failed_ids.contains(&trial_c),
            "trial C should be in failed list"
        );

        // Verify each trial has at least one failure description
        for (id, descs) in &result.failed_constraints {
            assert!(
                !descs.is_empty(),
                "trial {id} should have at least one failure description"
            );
        }
    }
}
