use common::TrialParams;
use config::AutoTunerConfig;

use crate::trial::TrialResult;

/// A single constraint failure with metric name, threshold, actual delta, and human description.
#[derive(Debug, Clone)]
pub struct ConstraintFailure {
    pub metric: String,
    pub threshold: f64,
    pub actual: f64,
    pub description: String,
}

/// The result of evaluating a trial variant against promotion constraints.
/// Collects all failures rather than short-circuiting on the first.
#[derive(Debug, Clone)]
pub struct ConstraintVerdict {
    pub failures: Vec<ConstraintFailure>,
}

impl ConstraintVerdict {
    /// Returns `true` if every constraint passed (no failures recorded).
    pub fn passes_all(&self) -> bool {
        self.failures.is_empty()
    }
}

/// Evaluates a completed `TrialResult` against a baseline `TrialResult` using
/// the promotion constraint thresholds stored in `AutoTunerConfig`.
///
/// Each metric is checked independently; all failures are collected and returned
/// in the `ConstraintVerdict` so callers can see exactly which constraints were
/// violated.
#[derive(Debug, Clone)]
pub struct ConstraintEvaluator {
    /// correction_rate must improve by at least this fraction (e.g. 0.05 = 5%).
    /// For correction_rate, *lower* is better.
    min_correction_improvement: f64,

    /// avg_tokens_per_message must not increase by more than this fraction.
    max_token_cost_increase: f64,

    /// avg_response_time_ms must not increase by more than this fraction.
    max_response_time_increase: f64,

    /// routing_stability must not decrease by more than this fraction.
    max_routing_stability_decrease: f64,

    /// memory_relevance must not decrease by more than this fraction.
    max_memory_relevance_decrease: f64,
}

impl ConstraintEvaluator {
    /// Build a `ConstraintEvaluator` from an `AutoTunerConfig`.
    pub fn from_config(config: &AutoTunerConfig) -> Self {
        Self {
            min_correction_improvement: config.min_correction_improvement,
            max_token_cost_increase: config.max_token_cost_increase,
            max_response_time_increase: config.max_response_time_increase,
            max_routing_stability_decrease: config.max_routing_stability_decrease,
            max_memory_relevance_decrease: config.max_memory_relevance_decrease,
        }
    }

    /// Evaluate `trial` against `baseline`.
    ///
    /// Returns a `ConstraintVerdict` containing every constraint that failed.
    /// An empty `failures` vec means the trial is eligible for promotion.
    pub fn evaluate(&self, trial: &TrialResult, baseline: &TrialResult) -> ConstraintVerdict {
        let mut failures = Vec::new();

        // --- correction_rate: lower is better; trial must improve by >= threshold ---
        if baseline.correction_rate > 0.0 {
            let improvement =
                (baseline.correction_rate - trial.correction_rate) / baseline.correction_rate;
            if improvement < self.min_correction_improvement {
                failures.push(ConstraintFailure {
                    metric: "correction_rate".into(),
                    threshold: self.min_correction_improvement,
                    actual: improvement,
                    description: format!(
                        "correction_rate improved by {:.1}% but requires >= {:.1}%",
                        improvement * 100.0,
                        self.min_correction_improvement * 100.0,
                    ),
                });
            }
        }

        // --- avg_tokens_per_message: lower is better; must not increase by > threshold ---
        if baseline.avg_tokens_per_message > 0.0 {
            let increase = (trial.avg_tokens_per_message - baseline.avg_tokens_per_message)
                / baseline.avg_tokens_per_message;
            if increase > self.max_token_cost_increase {
                failures.push(ConstraintFailure {
                    metric: "avg_tokens_per_message".into(),
                    threshold: self.max_token_cost_increase,
                    actual: increase,
                    description: format!(
                        "token cost increased by {:.1}% but max allowed is {:.1}%",
                        increase * 100.0,
                        self.max_token_cost_increase * 100.0,
                    ),
                });
            }
        }

        // --- avg_response_time_ms: lower is better; must not increase by > threshold ---
        if baseline.avg_response_time_ms > 0.0 {
            let increase = (trial.avg_response_time_ms - baseline.avg_response_time_ms)
                / baseline.avg_response_time_ms;
            if increase > self.max_response_time_increase {
                failures.push(ConstraintFailure {
                    metric: "avg_response_time_ms".into(),
                    threshold: self.max_response_time_increase,
                    actual: increase,
                    description: format!(
                        "response time increased by {:.1}% but max allowed is {:.1}%",
                        increase * 100.0,
                        self.max_response_time_increase * 100.0,
                    ),
                });
            }
        }

        // --- routing_stability: higher is better; must not decrease by > threshold ---
        if baseline.routing_stability > 0.0 {
            let decrease =
                (baseline.routing_stability - trial.routing_stability) / baseline.routing_stability;
            if decrease > self.max_routing_stability_decrease {
                failures.push(ConstraintFailure {
                    metric: "routing_stability".into(),
                    threshold: self.max_routing_stability_decrease,
                    actual: decrease,
                    description: format!(
                        "routing_stability decreased by {:.1}% but max allowed is {:.1}%",
                        decrease * 100.0,
                        self.max_routing_stability_decrease * 100.0,
                    ),
                });
            }
        }

        // --- memory_relevance: higher is better; must not decrease by > threshold ---
        if baseline.memory_relevance > 0.0 {
            let decrease =
                (baseline.memory_relevance - trial.memory_relevance) / baseline.memory_relevance;
            if decrease > self.max_memory_relevance_decrease {
                failures.push(ConstraintFailure {
                    metric: "memory_relevance".into(),
                    threshold: self.max_memory_relevance_decrease,
                    actual: decrease,
                    description: format!(
                        "memory_relevance decreased by {:.1}% but max allowed is {:.1}%",
                        decrease * 100.0,
                        self.max_memory_relevance_decrease * 100.0,
                    ),
                });
            }
        }

        ConstraintVerdict { failures }
    }
}

/// Euclidean distance between two `TrialParams` in parameter space.
///
/// Each `Option<f64>` field contributes its value when `Some`, or `0.0` when `None`.
/// Used to compute a diversity bonus: trials that are farther from the current
/// champion explore more of the parameter space, so they are preferred when
/// multiple trials pass all promotion constraints.
pub fn parameter_distance(a: &TrialParams, b: &TrialParams) -> f64 {
    let fields_a = trial_params_as_array(a);
    let fields_b = trial_params_as_array(b);

    let sum_sq: f64 = fields_a
        .iter()
        .zip(fields_b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum();

    sum_sq.sqrt()
}

/// Flatten all `Option<f64>` fields of `TrialParams` to a fixed-size array,
/// treating `None` as `0.0`.  The ordering must be stable so that distances are
/// comparable.
fn trial_params_as_array(p: &TrialParams) -> [f64; 16] {
    [
        p.skill_keyword_weight.unwrap_or(0.0),
        p.skill_semantic_weight.unwrap_or(0.0),
        p.skill_activation_threshold.unwrap_or(0.0),
        p.heuristic_confidence_threshold.unwrap_or(0.0),
        p.llm_classifier_timeout_ms.unwrap_or(0) as f64,
        p.relevance_weight_semantic.unwrap_or(0.0),
        p.relevance_weight_retrievability.unwrap_or(0.0),
        p.relevance_weight_situation.unwrap_or(0.0),
        p.fsrs_desired_retention.unwrap_or(0.0),
        p.accumulate_promote_threshold.map(|v| v as f64).unwrap_or(0.0),
        p.accumulate_min_days.map(|v| v as f64).unwrap_or(0.0),
        p.vector_top_k.map(|v| v as f64).unwrap_or(0.0),
        p.min_similarity.unwrap_or(0.0),
        p.relevance_weight_importance.unwrap_or(0.0),
        p.relevance_weight_frequency.unwrap_or(0.0),
        p.relevance_weight_temporal.unwrap_or(0.0),
    ]
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    fn default_evaluator() -> ConstraintEvaluator {
        ConstraintEvaluator::from_config(&AutoTunerConfig::default())
    }

    /// Build a baseline `TrialResult` with typical metric values.
    fn baseline() -> TrialResult {
        TrialResult {
            trial_id: Uuid::nil(),
            messages_scored: 200,
            correction_rate: 0.20, // 20% corrections
            classification_accuracy: 0.85,
            avg_tokens_per_message: 500.0,
            avg_response_time_ms: 800.0,
            routing_stability: 0.90,
            memory_relevance: 0.80,
            user_satisfaction: None,
            ..Default::default()
        }
    }

    #[test]
    fn passes_when_all_constraints_met() {
        let evaluator = default_evaluator();
        let b = baseline();

        // correction_rate: 10% improvement (baseline 0.20 → trial 0.18, (0.20-0.18)/0.20 = 10%)
        // tokens: +4% (500 → 520)
        // response time: +10% (800 → 880)
        // routing_stability: -5.5% (0.90 → 0.8505)
        // memory_relevance: -4.3% (0.80 → 0.7656)
        let trial = TrialResult {
            trial_id: Uuid::nil(),
            messages_scored: 200,
            correction_rate: 0.18,
            classification_accuracy: 0.85,
            avg_tokens_per_message: 520.0,
            avg_response_time_ms: 880.0,
            routing_stability: 0.8505,
            memory_relevance: 0.7656,
            user_satisfaction: None,
            ..Default::default()
        };

        let verdict = evaluator.evaluate(&trial, &b);
        assert!(
            verdict.passes_all(),
            "Expected all constraints to pass, but got failures: {:?}",
            verdict.failures,
        );
    }

    #[test]
    fn fails_when_correction_improvement_insufficient() {
        let evaluator = default_evaluator();
        let b = baseline();

        // correction_rate improvement = (0.20 - 0.195) / 0.20 = 2.5%, below the 5% threshold
        let trial = TrialResult {
            correction_rate: 0.195,
            avg_tokens_per_message: 520.0, // +4% — within limit
            avg_response_time_ms: 880.0,   // +10% — within limit
            routing_stability: 0.8505,     // -5.5% — within limit
            memory_relevance: 0.7656,      // -4.3% — within limit
            ..b.clone()
        };

        let verdict = evaluator.evaluate(&trial, &b);
        assert!(!verdict.passes_all(), "Expected a correction_rate failure");
        assert!(
            verdict
                .failures
                .iter()
                .any(|f| f.metric == "correction_rate"),
            "Expected correction_rate in failures, got: {:?}",
            verdict.failures,
        );
    }

    #[test]
    fn fails_when_token_cost_regresses() {
        let evaluator = default_evaluator();
        let b = baseline();

        // token cost: +10% (500 → 550), above the 8% threshold
        let trial = TrialResult {
            correction_rate: 0.18,         // 10% improvement — passes
            avg_tokens_per_message: 550.0, // +10% — FAILS
            avg_response_time_ms: 880.0,   // +10% — within limit
            routing_stability: 0.8505,     // -5.5% — within limit
            memory_relevance: 0.7656,      // -4.3% — within limit
            ..b.clone()
        };

        let verdict = evaluator.evaluate(&trial, &b);
        assert!(!verdict.passes_all(), "Expected a token cost failure");
        assert!(
            verdict
                .failures
                .iter()
                .any(|f| f.metric == "avg_tokens_per_message"),
            "Expected avg_tokens_per_message in failures, got: {:?}",
            verdict.failures,
        );
    }

    #[test]
    fn diversity_bonus_prefers_distant_variant() {
        let champion = TrialParams {
            skill_keyword_weight: Some(0.60),
            skill_semantic_weight: Some(0.40),
            skill_activation_threshold: Some(0.70),
            ..Default::default()
        };

        // Close to champion
        let close = TrialParams {
            skill_keyword_weight: Some(0.61),
            skill_semantic_weight: Some(0.39),
            skill_activation_threshold: Some(0.71),
            ..Default::default()
        };

        // Far from champion
        let far = TrialParams {
            skill_keyword_weight: Some(0.30),
            skill_semantic_weight: Some(0.70),
            skill_activation_threshold: Some(0.50),
            ..Default::default()
        };

        let dist_close = parameter_distance(&close, &champion);
        let dist_far = parameter_distance(&far, &champion);

        assert!(
            dist_far > dist_close,
            "Expected far ({dist_far:.4}) > close ({dist_close:.4})",
        );
    }
}
