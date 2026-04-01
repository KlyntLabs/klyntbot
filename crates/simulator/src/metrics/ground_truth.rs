use serde::{Deserialize, Serialize};

use crate::scenario::{CheckpointAssertion, Checkpoint, MetricName};
use super::{BaselineMetrics, MetricSnapshot};

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointResult {
    pub at_day: u32,
    pub assertions: Vec<AssertionResult>,
    pub all_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    pub description: String,
    pub passed: bool,
    pub actual_value: Option<f64>,
    pub expected: String,
}

// ---------------------------------------------------------------------------
// GroundTruthVerifier
// ---------------------------------------------------------------------------

pub struct GroundTruthVerifier;

impl GroundTruthVerifier {
    pub async fn verify_checkpoint(
        checkpoint: &Checkpoint,
        fact_repo: &cognitive::SemanticFactRepo,
        latest_snapshot: &MetricSnapshot,
        baselines: Option<&BaselineMetrics>,
    ) -> CheckpointResult {
        let mut results = Vec::with_capacity(checkpoint.assertions.len());

        for assertion in &checkpoint.assertions {
            let result = match assertion {
                CheckpointAssertion::FactExists {
                    subject,
                    predicate,
                    object,
                    min_confidence,
                } => {
                    Self::check_fact_exists(fact_repo, subject, predicate, object, *min_confidence)
                        .await
                }
                CheckpointAssertion::FactSuperseded {
                    subject,
                    predicate,
                    old_object,
                } => {
                    Self::check_fact_superseded(fact_repo, subject, predicate, old_object).await
                }
                CheckpointAssertion::MetricAbove { metric, threshold } => {
                    Self::check_metric_above(latest_snapshot, *metric, *threshold)
                }
                CheckpointAssertion::MetricImproved {
                    metric,
                    min_improvement_pct,
                } => Self::check_metric_improved(
                    latest_snapshot,
                    baselines,
                    *metric,
                    *min_improvement_pct,
                ),
            };
            results.push(result);
        }

        let all_passed = results.iter().all(|r| r.passed);

        CheckpointResult {
            at_day: checkpoint.at_day,
            assertions: results,
            all_passed,
        }
    }

    async fn check_fact_exists(
        fact_repo: &cognitive::SemanticFactRepo,
        subject: &str,
        predicate: &str,
        object: &str,
        min_confidence: f64,
    ) -> AssertionResult {
        let query = format!("{subject} {predicate} {object}");
        let facts = fact_repo.search_fts(&query, None, 10).await.unwrap_or_default();

        let found = facts.iter().find(|f| {
            f.subject == subject
                && f.predicate == predicate
                && f.object == object
                && f.superseded_at.is_none()
                && f.confidence >= min_confidence
        });

        AssertionResult {
            description: format!(
                "FactExists({subject}, {predicate}, {object}) >= {min_confidence}"
            ),
            passed: found.is_some(),
            actual_value: found.map(|f| f.confidence),
            expected: format!("confidence >= {min_confidence}"),
        }
    }

    async fn check_fact_superseded(
        fact_repo: &cognitive::SemanticFactRepo,
        subject: &str,
        predicate: &str,
        old_object: &str,
    ) -> AssertionResult {
        let query = format!("{subject} {predicate} {old_object}");
        let facts = fact_repo.search_fts(&query, None, 10).await.unwrap_or_default();

        let found = facts.iter().any(|f| {
            f.subject == subject
                && f.predicate == predicate
                && f.object == old_object
                && f.superseded_at.is_some()
        });

        AssertionResult {
            description: format!("FactSuperseded({subject}, {predicate}, {old_object})"),
            passed: found,
            actual_value: None,
            expected: "superseded_at is Some".to_string(),
        }
    }

    fn check_metric_above(
        snapshot: &MetricSnapshot,
        metric: MetricName,
        threshold: f64,
    ) -> AssertionResult {
        let value = get_metric_value(snapshot, &metric);

        AssertionResult {
            description: format!("MetricAbove({metric:?}, {threshold})"),
            passed: value >= threshold,
            actual_value: Some(value),
            expected: format!(">= {threshold}"),
        }
    }

    fn check_metric_improved(
        snapshot: &MetricSnapshot,
        baselines: Option<&BaselineMetrics>,
        metric: MetricName,
        min_improvement_pct: f64,
    ) -> AssertionResult {
        let current = get_metric_value(snapshot, &metric);
        let baseline = baselines
            .map(|bl| get_baseline_value(bl, &metric))
            .unwrap_or(0.0);

        let improvement = if baseline.abs() < f64::EPSILON {
            0.0
        } else {
            (current - baseline) / baseline * 100.0
        };

        AssertionResult {
            description: format!("MetricImproved({metric:?}, {min_improvement_pct}%)"),
            passed: improvement >= min_improvement_pct,
            actual_value: Some(improvement),
            expected: format!(">= {min_improvement_pct}% improvement"),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn get_metric_value(snapshot: &MetricSnapshot, metric: &MetricName) -> f64 {
    match metric {
        MetricName::KnowledgeRetention => snapshot.knowledge_retention,
        MetricName::RetrievalPrecision => snapshot.retrieval_precision,
        MetricName::RetrievalRecall => snapshot.retrieval_recall,
        MetricName::FactExtractionAccuracy => snapshot.fact_extraction_accuracy,
        MetricName::ContradictionDetectionRate => snapshot.contradiction_detection_rate,
        MetricName::CorrectionRate => snapshot.correction_rate,
        MetricName::TokenEfficiency => snapshot.token_efficiency,
        MetricName::PersonalizationScore => snapshot.personalization_score,
        MetricName::TaskCompletionRate => snapshot.task_completion_rate,
        MetricName::RoutingStability => snapshot.routing_stability,
        MetricName::InsightUsefulness => snapshot.insight_usefulness,
        MetricName::AutotunerPromotionSuccess => snapshot.autotuner_promotion_success,
        MetricName::CommunityStability => snapshot.community_stability,
        MetricName::BrainVersionVelocity => snapshot.brain_version_velocity as f64,
    }
}

fn get_baseline_value(baselines: &BaselineMetrics, metric: &MetricName) -> f64 {
    match metric {
        // Tier 2 — behavioral quality (baselines available)
        MetricName::TokenEfficiency => baselines.token_efficiency,
        MetricName::PersonalizationScore => baselines.personalization_score,
        MetricName::TaskCompletionRate => baselines.task_completion_rate,
        MetricName::RoutingStability => baselines.routing_stability,
        MetricName::InsightUsefulness => baselines.insight_usefulness,
        // Tier 1 and Tier 3 — no baselines tracked
        _ => 0.0,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_metric_value_maps_all_fields() {
        let snap = MetricSnapshot {
            knowledge_retention: 0.9,
            retrieval_precision: 0.8,
            retrieval_recall: 0.7,
            fact_extraction_accuracy: 0.85,
            contradiction_detection_rate: 0.3,
            correction_rate: 0.1,
            token_efficiency: 500.0,
            personalization_score: 0.75,
            task_completion_rate: 0.6,
            routing_stability: 0.95,
            insight_usefulness: 0.5,
            autotuner_promotion_success: 0.4,
            community_stability: 0.65,
            brain_version_velocity: 3,
            ..Default::default()
        };

        assert!((get_metric_value(&snap, &MetricName::KnowledgeRetention) - 0.9).abs() < 1e-9);
        assert!((get_metric_value(&snap, &MetricName::RetrievalPrecision) - 0.8).abs() < 1e-9);
        assert!((get_metric_value(&snap, &MetricName::RetrievalRecall) - 0.7).abs() < 1e-9);
        assert!(
            (get_metric_value(&snap, &MetricName::FactExtractionAccuracy) - 0.85).abs() < 1e-9
        );
        assert!(
            (get_metric_value(&snap, &MetricName::ContradictionDetectionRate) - 0.3).abs() < 1e-9
        );
        assert!((get_metric_value(&snap, &MetricName::CorrectionRate) - 0.1).abs() < 1e-9);
        assert!((get_metric_value(&snap, &MetricName::TokenEfficiency) - 500.0).abs() < 1e-9);
        assert!(
            (get_metric_value(&snap, &MetricName::PersonalizationScore) - 0.75).abs() < 1e-9
        );
        assert!((get_metric_value(&snap, &MetricName::TaskCompletionRate) - 0.6).abs() < 1e-9);
        assert!((get_metric_value(&snap, &MetricName::RoutingStability) - 0.95).abs() < 1e-9);
        assert!((get_metric_value(&snap, &MetricName::InsightUsefulness) - 0.5).abs() < 1e-9);
        assert!(
            (get_metric_value(&snap, &MetricName::AutotunerPromotionSuccess) - 0.4).abs() < 1e-9
        );
        assert!((get_metric_value(&snap, &MetricName::CommunityStability) - 0.65).abs() < 1e-9);
        assert!((get_metric_value(&snap, &MetricName::BrainVersionVelocity) - 3.0).abs() < 1e-9);
    }

    #[test]
    fn get_baseline_value_returns_tier2_only() {
        let baselines = BaselineMetrics {
            token_efficiency: 100.0,
            personalization_score: 0.8,
            task_completion_rate: 0.7,
            routing_stability: 0.9,
            insight_usefulness: 0.6,
        };

        assert!((get_baseline_value(&baselines, &MetricName::TokenEfficiency) - 100.0).abs() < 1e-9);
        assert!(
            (get_baseline_value(&baselines, &MetricName::PersonalizationScore) - 0.8).abs() < 1e-9
        );
        assert!(
            (get_baseline_value(&baselines, &MetricName::TaskCompletionRate) - 0.7).abs() < 1e-9
        );
        assert!((get_baseline_value(&baselines, &MetricName::RoutingStability) - 0.9).abs() < 1e-9);
        assert!(
            (get_baseline_value(&baselines, &MetricName::InsightUsefulness) - 0.6).abs() < 1e-9
        );

        // Tier 1 and Tier 3 return 0.0
        assert!((get_baseline_value(&baselines, &MetricName::KnowledgeRetention) - 0.0).abs() < 1e-9);
        assert!(
            (get_baseline_value(&baselines, &MetricName::AutotunerPromotionSuccess) - 0.0).abs()
                < 1e-9
        );
    }

    #[test]
    fn metric_above_passes_when_value_meets_threshold() {
        let snap = MetricSnapshot {
            knowledge_retention: 0.85,
            ..Default::default()
        };

        let result =
            GroundTruthVerifier::check_metric_above(&snap, MetricName::KnowledgeRetention, 0.8);
        assert!(result.passed);
        assert!((result.actual_value.unwrap() - 0.85).abs() < 1e-9);
    }

    #[test]
    fn metric_above_fails_when_value_below_threshold() {
        let snap = MetricSnapshot {
            knowledge_retention: 0.5,
            ..Default::default()
        };

        let result =
            GroundTruthVerifier::check_metric_above(&snap, MetricName::KnowledgeRetention, 0.8);
        assert!(!result.passed);
    }

    #[test]
    fn metric_improved_computes_percentage_correctly() {
        let snap = MetricSnapshot {
            task_completion_rate: 0.84,
            ..Default::default()
        };
        let baselines = BaselineMetrics {
            task_completion_rate: 0.7,
            ..Default::default()
        };

        let result = GroundTruthVerifier::check_metric_improved(
            &snap,
            Some(&baselines),
            MetricName::TaskCompletionRate,
            15.0,
        );

        // improvement = (0.84 - 0.7) / 0.7 * 100 = 20.0%
        assert!(result.passed);
        assert!((result.actual_value.unwrap() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn metric_improved_fails_when_insufficient_improvement() {
        let snap = MetricSnapshot {
            task_completion_rate: 0.72,
            ..Default::default()
        };
        let baselines = BaselineMetrics {
            task_completion_rate: 0.7,
            ..Default::default()
        };

        let result = GroundTruthVerifier::check_metric_improved(
            &snap,
            Some(&baselines),
            MetricName::TaskCompletionRate,
            10.0,
        );

        // improvement = (0.72 - 0.7) / 0.7 * 100 ~= 2.86%
        assert!(!result.passed);
    }

    #[test]
    fn metric_improved_zero_baseline_does_not_panic() {
        let snap = MetricSnapshot {
            knowledge_retention: 0.5,
            ..Default::default()
        };

        // No baselines for tier-1 metric => baseline is 0.0
        let baselines = BaselineMetrics::default();
        let result = GroundTruthVerifier::check_metric_improved(
            &snap,
            Some(&baselines),
            MetricName::KnowledgeRetention,
            10.0,
        );

        // Zero baseline should not panic; improvement = 0.0
        assert!(!result.passed);
        assert!((result.actual_value.unwrap() - 0.0).abs() < 1e-9);
    }

    #[test]
    fn metric_improved_none_baselines() {
        let snap = MetricSnapshot {
            task_completion_rate: 0.8,
            ..Default::default()
        };

        let result = GroundTruthVerifier::check_metric_improved(
            &snap,
            None,
            MetricName::TaskCompletionRate,
            10.0,
        );

        // No baselines => baseline 0.0 => improvement 0.0
        assert!(!result.passed);
    }
}
