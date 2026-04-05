pub mod behavioral;
pub mod cognitive;
pub mod cost;
pub mod ground_truth;
pub mod memory;
pub mod system;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// MetricSnapshot — one epoch's worth of metrics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricSnapshot {
    pub epoch: DateTime<Utc>,
    // Tier 1 — memory fidelity
    pub knowledge_retention: f64,
    pub retrieval_precision: f64,
    pub retrieval_recall: f64,
    pub fact_extraction_accuracy: f64,
    pub contradiction_detection_rate: f64,
    pub correction_rate: f64,
    // Tier 2 — behavioral quality
    pub token_efficiency: f64,
    pub personalization_score: f64,
    pub task_completion_rate: f64,
    pub routing_stability: f64,
    pub routing_accuracy: f64,
    pub response_quality: f64,
    pub salience_extract_rate: f64,
    pub insight_usefulness: f64,
    // Tier 3 — system health
    pub autotuner_promotion_success: f64,
    pub community_stability: f64,
    pub brain_version_velocity: u32,
    // Tier 4 — cognitive depth
    pub memory_retrievability: f64,
    pub meta_rule_count: u32,
    // Tier 5 — agent path metrics
    pub agent_routing_accuracy: f64,
    pub agent_tool_selection: f64,
    pub agent_mode_distribution: f64,
    pub react_convergence_rate: f64,
    pub agent_response_quality: f64,
    // Tier 6 — multi-turn, cross-feature, adversarial
    pub multi_turn_coherence: f64,
    pub cross_feature_chain_success: f64,
    pub adversarial_resilience: f64,
    pub error_recovery_rate: f64,
    // Performance
    pub wall_time_per_epoch_ms: f64,
    // Tier 7 — cost economics
    pub cost_per_outcome_usd: f64,
    pub cache_hit_rate: f64,
    // Tier 4 — cognitive depth (extended)
    pub retrievability_min: f64,
    pub retrievability_p25: f64,
    // Tier 2 — behavioral quality (extended)
    pub estimation_deviation_avg: f64,
}

// ---------------------------------------------------------------------------
// BaselineMetrics — averages computed after the warm-up period
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BaselineMetrics {
    pub token_efficiency: f64,
    pub personalization_score: f64,
    pub task_completion_rate: f64,
    pub routing_stability: f64,
    pub routing_accuracy: f64,
    pub insight_usefulness: f64,
    // Tier 1 — memory fidelity baselines
    pub knowledge_retention: f64,
    pub retrieval_precision: f64,
    pub retrieval_recall: f64,
    pub fact_extraction_accuracy: f64,
    pub cost_per_outcome_usd: f64,
    pub cache_hit_rate: f64,
}

// ---------------------------------------------------------------------------
// RegressionAlert — emitted when a metric regresses beyond threshold
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionAlert {
    pub metric: String,
    pub baseline: f64,
    pub current: f64,
    pub regression_pct: f64,
}

// ---------------------------------------------------------------------------
// EpochAccumulator — raw counters accumulated during one epoch
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct EpochAccumulator {
    pub messages_processed: u32,
    pub corrections: u32,
    pub facts_introduced: u32,
    pub facts_extracted: u32,
    pub contradictions_detected: u32,
    pub total_tokens: u64,
    pub retrieval_precision_sum: f64,
    pub retrieval_recall_sum: f64,
    pub retrieval_count: u32,
    pub routing_matches: u32,
    pub routing_correct: u32,
    pub routing_total: u32,
    pub tasks_created: u32,
    pub tasks_completed: u32,
    pub facts_superseded: u32,
    pub salience_extract: u32,
    pub salience_accumulate: u32,
    pub salience_discard: u32,
    pub response_quality_sum: f64,
    pub response_quality_count: u32,
    // Agent path counters
    pub agent_calls: u32,
    pub agent_successful: u32,
    pub agent_routing_correct: u32,
    pub agent_routing_total: u32,
    pub agent_tool_selection_correct: u32,
    pub agent_tool_selection_total: u32,
    pub agent_tool_calls: u32,
    pub agent_reactive_count: u32,
    pub agent_react_converged: u32,
    pub agent_react_iterations_sum: u32,
    pub agent_response_quality_sum: f64,
    pub agent_response_quality_count: u32,
    // Tier 6 — multi-turn, cross-feature, adversarial (added early for compilation)
    pub multi_turn_coherence_sum: f64,
    pub multi_turn_coherence_count: u32,
    pub cross_feature_success: u32,
    pub cross_feature_total: u32,
    pub adversarial_resilient: u32,
    pub adversarial_total: u32,
    pub error_recovered: u32,
    pub error_injected: u32,
    // Cost tracking
    pub total_cost_usd: f64,
    pub total_prompt_tokens: u64,
    pub total_cache_read_tokens: u64,
    // Estimation tracking
    pub estimation_deviation_sum: f64,
    pub estimation_count: u32,
}

// ---------------------------------------------------------------------------
// MetricCollector — accumulates per-epoch data and produces snapshots
// ---------------------------------------------------------------------------

pub struct MetricCollector {
    pub timeline: Vec<MetricSnapshot>,
    pub baselines: Option<BaselineMetrics>,
    baseline_day: u32,
    accumulator: EpochAccumulator,
    cumulative_tasks_created: u32,
    cumulative_tasks_completed: u32,
    cumulative_facts_superseded: u32,
    cumulative_facts_introduced: u32,
    cumulative_contradictions: u32,
}

impl MetricCollector {
    /// Create a new collector. Baselines are computed once `day >= baseline_after_day`.
    /// Typical value: 30 (compute baselines after the first 30 simulation days).
    pub fn new(baseline_after_day: u32) -> Self {
        Self {
            timeline: Vec::new(),
            baselines: None,
            baseline_day: baseline_after_day,
            accumulator: EpochAccumulator::default(),
            cumulative_tasks_created: 0,
            cumulative_tasks_completed: 0,
            cumulative_facts_superseded: 0,
            cumulative_facts_introduced: 0,
            cumulative_contradictions: 0,
        }
    }

    /// Mutable access to the current epoch's accumulator so callers can bump
    /// counters as events happen during the epoch.
    pub fn accumulator_mut(&mut self) -> &mut EpochAccumulator {
        &mut self.accumulator
    }

    pub fn total_facts_superseded(&self) -> u32 {
        self.cumulative_facts_superseded
    }

    /// Finalise the current epoch: compute derived rates from the accumulator,
    /// push a [`MetricSnapshot`], optionally compute baselines, then reset the
    /// accumulator for the next epoch.
    #[allow(clippy::too_many_arguments)]
    pub fn snapshot(
        &mut self,
        epoch: DateTime<Utc>,
        day: u32,
        knowledge_retention: f64,
        autotuner_promotion_success: f64,
        community_stability: f64,
        brain_version_velocity: u32,
        insight_usefulness: f64,
        wall_time_ms: f64,
    ) {
        self.cumulative_tasks_created += self.accumulator.tasks_created;
        self.cumulative_tasks_completed += self.accumulator.tasks_completed;
        self.cumulative_facts_superseded += self.accumulator.facts_superseded;
        self.cumulative_facts_introduced += self.accumulator.facts_introduced;
        self.cumulative_contradictions += self.accumulator.contradictions_detected;

        let acc = &self.accumulator;
        let msgs = acc.messages_processed.max(1) as f64;

        let correction_rate = acc.corrections as f64 / msgs;
        let fact_extraction_accuracy = if acc.facts_introduced == 0 {
            1.0
        } else {
            acc.facts_extracted as f64 / acc.facts_introduced as f64
        };
        let contradiction_detection_rate = if self.cumulative_facts_introduced == 0 {
            0.0
        } else {
            self.cumulative_contradictions as f64 / self.cumulative_facts_introduced as f64
        };

        let retrieval_precision = if acc.retrieval_count == 0 {
            0.0
        } else {
            acc.retrieval_precision_sum / acc.retrieval_count as f64
        };
        let retrieval_recall = if acc.retrieval_count == 0 {
            0.0
        } else {
            acc.retrieval_recall_sum / acc.retrieval_count as f64
        };

        let token_efficiency = acc.total_tokens as f64 / msgs;
        let task_completion_rate = if self.cumulative_tasks_created == 0 {
            0.0
        } else {
            (self.cumulative_tasks_completed as f64 / self.cumulative_tasks_created as f64).min(1.0)
        };
        let routing_stability = acc.routing_matches as f64 / msgs;
        // In the flat skill system, routing accuracy comes from agent path
        // (all messages handled by "klyntbot"). Fall back to heuristic metric
        // if agent routing data is absent.
        let routing_accuracy = if acc.agent_routing_total > 0 {
            acc.agent_routing_correct as f64 / acc.agent_routing_total as f64
        } else if acc.routing_total > 0 {
            acc.routing_correct as f64 / acc.routing_total as f64
        } else {
            0.0
        };

        // Prefer agent-path response quality over heuristic-mode metric.
        let response_quality = if acc.agent_response_quality_count > 0 {
            acc.agent_response_quality_sum / acc.agent_response_quality_count as f64
        } else if acc.response_quality_count > 0 {
            acc.response_quality_sum / acc.response_quality_count as f64
        } else {
            0.0
        };
        let salience_total = acc.salience_extract + acc.salience_accumulate + acc.salience_discard;
        let salience_extract_rate = if salience_total == 0 {
            0.0
        } else {
            acc.salience_extract as f64 / salience_total as f64
        };

        let personalization_score = behavioral::personalization_score(
            knowledge_retention,
            retrieval_precision,
            retrieval_recall,
        );

        // Agent path metrics
        let agent_routing_accuracy = if acc.agent_routing_total == 0 {
            0.0
        } else {
            acc.agent_routing_correct as f64 / acc.agent_routing_total as f64
        };
        let agent_tool_selection = if acc.agent_tool_selection_total == 0 {
            0.0
        } else {
            acc.agent_tool_selection_correct as f64 / acc.agent_tool_selection_total as f64
        };
        let agent_mode_distribution = if acc.agent_calls == 0 {
            0.0
        } else {
            acc.agent_reactive_count as f64 / acc.agent_calls as f64
        };
        let react_convergence_rate = if acc.agent_reactive_count == 0 {
            0.0
        } else {
            acc.agent_react_converged as f64 / acc.agent_reactive_count as f64
        };
        let agent_response_quality = if acc.agent_response_quality_count == 0 {
            0.0
        } else {
            acc.agent_response_quality_sum / acc.agent_response_quality_count as f64
        };

        let multi_turn_coherence = if acc.multi_turn_coherence_count == 0 {
            0.0
        } else {
            acc.multi_turn_coherence_sum / acc.multi_turn_coherence_count as f64
        };
        let cross_feature_chain_success = if acc.cross_feature_total == 0 {
            0.0
        } else {
            acc.cross_feature_success as f64 / acc.cross_feature_total as f64
        };
        let adversarial_resilience = if acc.adversarial_total == 0 {
            0.0
        } else {
            acc.adversarial_resilient as f64 / acc.adversarial_total as f64
        };
        let error_recovery_rate = if acc.error_injected == 0 {
            0.0
        } else {
            acc.error_recovered as f64 / acc.error_injected as f64
        };

        let snap = MetricSnapshot {
            epoch,
            knowledge_retention,
            retrieval_precision,
            retrieval_recall,
            fact_extraction_accuracy,
            contradiction_detection_rate,
            correction_rate,
            token_efficiency,
            personalization_score,
            task_completion_rate,
            routing_stability,
            routing_accuracy,
            response_quality,
            salience_extract_rate,
            insight_usefulness,
            autotuner_promotion_success,
            community_stability,
            brain_version_velocity,
            agent_routing_accuracy,
            agent_tool_selection,
            agent_mode_distribution,
            react_convergence_rate,
            agent_response_quality,
            multi_turn_coherence,
            cross_feature_chain_success,
            adversarial_resilience,
            error_recovery_rate,
            memory_retrievability: 0.0,
            meta_rule_count: 0,
            wall_time_per_epoch_ms: wall_time_ms,
            // Populated post-snapshot via update_latest_cost_and_estimation
            cost_per_outcome_usd: 0.0,
            cache_hit_rate: 0.0,
            retrievability_min: 0.0,
            retrievability_p25: 0.0,
            estimation_deviation_avg: 0.0,
        };

        self.timeline.push(snap);

        // Compute baselines once after the warm-up period.
        if self.baselines.is_none() && day >= self.baseline_day {
            self.compute_baselines();
        }

        // Reset accumulator for the next epoch.
        self.accumulator = EpochAccumulator::default();
    }

    /// Update the latest snapshot with cognitive metrics computed externally.
    pub fn update_latest_cognitive(&mut self, memory_retrievability: f64, meta_rule_count: u32) {
        if let Some(snap) = self.timeline.last_mut() {
            snap.memory_retrievability = memory_retrievability;
            snap.meta_rule_count = meta_rule_count;
        }
    }

    /// Update the latest snapshot with cost and estimation metrics computed externally.
    pub fn update_latest_cost_and_estimation(
        &mut self,
        cost_per_outcome_usd: f64,
        cache_hit_rate: f64,
        retrievability_min: f64,
        retrievability_p25: f64,
        estimation_deviation_avg: f64,
    ) {
        if let Some(snap) = self.timeline.last_mut() {
            snap.cost_per_outcome_usd = cost_per_outcome_usd;
            snap.cache_hit_rate = cache_hit_rate;
            snap.retrievability_min = retrievability_min;
            snap.retrievability_p25 = retrievability_p25;
            snap.estimation_deviation_avg = estimation_deviation_avg;
        }
    }

    /// Average all timeline snapshots to establish tier-2 baselines.
    fn compute_baselines(&mut self) {
        if self.timeline.is_empty() {
            return;
        }
        let n = self.timeline.len() as f64;

        let mut bl = BaselineMetrics::default();
        for s in &self.timeline {
            bl.token_efficiency += s.token_efficiency;
            bl.personalization_score += s.personalization_score;
            bl.task_completion_rate += s.task_completion_rate;
            bl.routing_stability += s.routing_stability;
            bl.routing_accuracy += s.routing_accuracy;
            bl.insight_usefulness += s.insight_usefulness;
            bl.knowledge_retention += s.knowledge_retention;
            bl.retrieval_precision += s.retrieval_precision;
            bl.retrieval_recall += s.retrieval_recall;
            bl.fact_extraction_accuracy += s.fact_extraction_accuracy;
            bl.cost_per_outcome_usd += s.cost_per_outcome_usd;
            bl.cache_hit_rate += s.cache_hit_rate;
        }
        bl.token_efficiency /= n;
        bl.personalization_score /= n;
        bl.task_completion_rate /= n;
        bl.routing_stability /= n;
        bl.routing_accuracy /= n;
        bl.insight_usefulness /= n;
        bl.knowledge_retention /= n;
        bl.retrieval_precision /= n;
        bl.retrieval_recall /= n;
        bl.fact_extraction_accuracy /= n;
        bl.cost_per_outcome_usd /= n;
        bl.cache_hit_rate /= n;

        self.baselines = Some(bl);
    }

    /// Compare the latest snapshot against baselines.
    ///
    /// `threshold_pct` is the percentage drop (or increase for token_efficiency)
    /// that triggers an alert (e.g. 10.0 means 10%).
    ///
    /// For **token_efficiency** a regression means the value *increased* (more
    /// tokens per message is worse). For all other metrics, a regression means
    /// the value *decreased*.
    pub fn check_regressions(&self, threshold_pct: f64) -> Vec<RegressionAlert> {
        let Some(bl) = &self.baselines else {
            return Vec::new();
        };
        let Some(latest) = self.timeline.last() else {
            return Vec::new();
        };

        let mut alerts = Vec::new();

        // Token efficiency: regression = increase.
        if bl.token_efficiency > 0.0 {
            let pct = (latest.token_efficiency - bl.token_efficiency) / bl.token_efficiency * 100.0;
            if pct > threshold_pct {
                alerts.push(RegressionAlert {
                    metric: "token_efficiency".into(),
                    baseline: bl.token_efficiency,
                    current: latest.token_efficiency,
                    regression_pct: pct,
                });
            }
        }

        // Cost per outcome: regression = increase.
        if bl.cost_per_outcome_usd > 0.0 {
            let pct = (latest.cost_per_outcome_usd - bl.cost_per_outcome_usd)
                / bl.cost_per_outcome_usd
                * 100.0;
            if pct > threshold_pct {
                alerts.push(RegressionAlert {
                    metric: "cost_per_outcome_usd".into(),
                    baseline: bl.cost_per_outcome_usd,
                    current: latest.cost_per_outcome_usd,
                    regression_pct: pct,
                });
            }
        }

        // Other metrics: regression = decrease.
        // Fourth tuple element: skip regression check when current is 0.0
        // (the metric is undefined for that epoch, not regressed).
        let checks: &[(&str, f64, f64, bool)] = &[
            (
                "personalization_score",
                bl.personalization_score,
                latest.personalization_score,
                false,
            ),
            (
                "task_completion_rate",
                bl.task_completion_rate,
                latest.task_completion_rate,
                true,
            ),
            (
                "routing_stability",
                bl.routing_stability,
                latest.routing_stability,
                false,
            ),
            (
                "routing_accuracy",
                bl.routing_accuracy,
                latest.routing_accuracy,
                false,
            ),
            (
                "insight_usefulness",
                bl.insight_usefulness,
                latest.insight_usefulness,
                false,
            ),
            (
                "knowledge_retention",
                bl.knowledge_retention,
                latest.knowledge_retention,
                false,
            ),
            (
                "retrieval_precision",
                bl.retrieval_precision,
                latest.retrieval_precision,
                true,
            ),
            (
                "retrieval_recall",
                bl.retrieval_recall,
                latest.retrieval_recall,
                true,
            ),
            (
                "fact_extraction_accuracy",
                bl.fact_extraction_accuracy,
                latest.fact_extraction_accuracy,
                false,
            ),
            (
                "cache_hit_rate",
                bl.cache_hit_rate,
                latest.cache_hit_rate,
                true, // skip when 0.0
            ),
        ];

        for &(name, baseline, current, skip_when_zero) in checks {
            if baseline > 0.0 {
                if skip_when_zero && current == 0.0 {
                    continue;
                }
                let pct = (baseline - current) / baseline * 100.0;
                if pct > threshold_pct {
                    alerts.push(RegressionAlert {
                        metric: name.into(),
                        baseline,
                        current,
                        regression_pct: pct,
                    });
                }
            }
        }

        alerts
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn utc(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
    }

    #[test]
    fn snapshot_computes_rates_correctly() {
        let mut collector = MetricCollector::new(30);

        {
            let acc = collector.accumulator_mut();
            acc.messages_processed = 10;
            acc.corrections = 2;
            acc.facts_introduced = 5;
            acc.facts_extracted = 4;
            acc.contradictions_detected = 1;
            acc.total_tokens = 5000;
            acc.retrieval_precision_sum = 2.4;
            acc.retrieval_recall_sum = 1.8;
            acc.retrieval_count = 3;
            acc.routing_matches = 8;
            acc.routing_correct = 7;
            acc.routing_total = 10;
            acc.tasks_created = 4;
            acc.tasks_completed = 3;
        }

        let epoch = utc(2026, 4, 1, 12, 0);
        collector.snapshot(
            epoch, 1,      // day
            0.85,   // knowledge_retention
            0.9,    // autotuner_promotion_success
            0.75,   // community_stability
            2,      // brain_version_velocity
            0.6,    // insight_usefulness
            1500.0, // wall_time_ms
        );

        assert_eq!(collector.timeline.len(), 1);
        let snap = &collector.timeline[0];

        // correction_rate = 2 / 10 = 0.2
        assert!((snap.correction_rate - 0.2).abs() < 1e-9);

        // fact_extraction_accuracy = 4 / 5 = 0.8
        assert!((snap.fact_extraction_accuracy - 0.8).abs() < 1e-9);

        // contradiction_detection_rate = 1 / 5 = 0.2
        assert!((snap.contradiction_detection_rate - 0.2).abs() < 1e-9);

        // retrieval_precision = 2.4 / 3 = 0.8
        assert!((snap.retrieval_precision - 0.8).abs() < 1e-9);

        // retrieval_recall = 1.8 / 3 = 0.6
        assert!((snap.retrieval_recall - 0.6).abs() < 1e-9);

        // token_efficiency = 5000 / 10 = 500.0
        assert!((snap.token_efficiency - 500.0).abs() < 1e-9);

        // task_completion_rate = 3 / 4 = 0.75
        assert!((snap.task_completion_rate - 0.75).abs() < 1e-9);

        // routing_stability = 8 / 10 = 0.8
        assert!((snap.routing_stability - 0.8).abs() < 1e-9);

        // routing_accuracy = 7 / 10 = 0.7
        assert!((snap.routing_accuracy - 0.7).abs() < 1e-9);

        // personalization_score = 0.85 * 0.4 + 0.8 * 0.3 + 0.6 * 0.3
        //                       = 0.34 + 0.24 + 0.18 = 0.76
        assert!((snap.personalization_score - 0.76).abs() < 1e-9);

        // Pass-through values
        assert!((snap.knowledge_retention - 0.85).abs() < 1e-9);
        assert!((snap.autotuner_promotion_success - 0.9).abs() < 1e-9);
        assert!((snap.community_stability - 0.75).abs() < 1e-9);
        assert_eq!(snap.brain_version_velocity, 2);
        assert!((snap.insight_usefulness - 0.6).abs() < 1e-9);
        assert!((snap.wall_time_per_epoch_ms - 1500.0).abs() < 1e-9);

        // Accumulator should be reset.
        assert_eq!(collector.accumulator_mut().messages_processed, 0);
    }

    #[test]
    fn task_completion_rate_is_cumulative() {
        let mut collector = MetricCollector::new(30);

        // Epoch 1: 4 tasks created, 0 completed → rate = 0/4 = 0.0
        {
            let acc = collector.accumulator_mut();
            acc.messages_processed = 5;
            acc.tasks_created = 4;
            acc.tasks_completed = 0;
        }
        collector.snapshot(utc(2026, 4, 1, 12, 0), 1, 0.8, 0.5, 0.5, 1, 0.5, 100.0);
        assert!(
            (collector.timeline[0].task_completion_rate - 0.0).abs() < 1e-9,
            "epoch 1: expected 0.0, got {}",
            collector.timeline[0].task_completion_rate
        );

        // Epoch 2: 0 created, 3 completed → cumulative 4 created, 3 completed → rate = 3/4 = 0.75
        {
            let acc = collector.accumulator_mut();
            acc.messages_processed = 5;
            acc.tasks_created = 0;
            acc.tasks_completed = 3;
        }
        collector.snapshot(utc(2026, 4, 2, 12, 0), 2, 0.8, 0.5, 0.5, 1, 0.5, 100.0);
        assert!(
            (collector.timeline[1].task_completion_rate - 0.75).abs() < 1e-9,
            "epoch 2: expected 0.75, got {}",
            collector.timeline[1].task_completion_rate
        );

        // Epoch 3: 2 created, 1 completed → cumulative 6 created, 4 completed → rate = 4/6 ≈ 0.6667
        {
            let acc = collector.accumulator_mut();
            acc.messages_processed = 5;
            acc.tasks_created = 2;
            acc.tasks_completed = 1;
        }
        collector.snapshot(utc(2026, 4, 3, 12, 0), 3, 0.8, 0.5, 0.5, 1, 0.5, 100.0);
        let expected = 4.0 / 6.0;
        assert!(
            (collector.timeline[2].task_completion_rate - expected).abs() < 1e-9,
            "epoch 3: expected {expected}, got {}",
            collector.timeline[2].task_completion_rate
        );
    }

    #[test]
    fn baselines_computed_after_threshold_day() {
        let mut collector = MetricCollector::new(2);

        // Day 1 — baselines not yet computed.
        {
            let acc = collector.accumulator_mut();
            acc.messages_processed = 10;
            acc.total_tokens = 1000;
            acc.routing_matches = 8;
        }
        collector.snapshot(utc(2026, 4, 1, 12, 0), 1, 0.8, 0.5, 0.5, 1, 0.5, 100.0);
        assert!(collector.baselines.is_none());

        // Day 2 — baselines should be computed now.
        {
            let acc = collector.accumulator_mut();
            acc.messages_processed = 10;
            acc.total_tokens = 2000;
            acc.routing_matches = 6;
        }
        collector.snapshot(utc(2026, 4, 2, 12, 0), 2, 0.9, 0.6, 0.6, 2, 0.7, 120.0);
        assert!(collector.baselines.is_some());

        let bl = collector.baselines.as_ref().unwrap();
        // Average of two snapshots:
        // token_efficiency: (100.0 + 200.0) / 2 = 150.0
        assert!((bl.token_efficiency - 150.0).abs() < 1e-9);
    }

    #[test]
    fn regression_detection_works() {
        let mut collector = MetricCollector::new(1);

        // Day 1 — establish baselines.
        {
            let acc = collector.accumulator_mut();
            acc.messages_processed = 10;
            acc.total_tokens = 1000;
            acc.routing_matches = 9;
            acc.tasks_created = 10;
            acc.tasks_completed = 8;
        }
        collector.snapshot(utc(2026, 4, 1, 12, 0), 1, 0.9, 0.5, 0.5, 1, 0.8, 100.0);

        // Day 2 — regress: more tokens, lower routing stability.
        {
            let acc = collector.accumulator_mut();
            acc.messages_processed = 10;
            acc.total_tokens = 2000; // doubled
            acc.routing_matches = 4; // dropped from 9 to 4
            acc.tasks_created = 10;
            acc.tasks_completed = 8;
        }
        collector.snapshot(utc(2026, 4, 2, 12, 0), 2, 0.9, 0.5, 0.5, 1, 0.8, 100.0);

        let alerts = collector.check_regressions(10.0);
        let alert_names: Vec<&str> = alerts.iter().map(|a| a.metric.as_str()).collect();

        // Token efficiency went from 100 to 200 — 100% increase.
        assert!(alert_names.contains(&"token_efficiency"));
        // Routing stability went from 0.9 to 0.4 — >50% decrease.
        assert!(alert_names.contains(&"routing_stability"));
    }

    #[test]
    fn contradiction_rate_is_cumulative() {
        let mut collector = MetricCollector::new(30);

        // Epoch 1: 2 facts introduced, 1 contradiction
        {
            let acc = collector.accumulator_mut();
            acc.messages_processed = 5;
            acc.facts_introduced = 2;
            acc.contradictions_detected = 1;
        }
        collector.snapshot(utc(2026, 4, 1, 12, 0), 1, 0.8, 0.5, 0.5, 1, 0.5, 100.0);
        assert!(
            (collector.timeline[0].contradiction_detection_rate - 0.5).abs() < 1e-9,
            "epoch 1: expected 0.5, got {}",
            collector.timeline[0].contradiction_detection_rate
        );

        // Epoch 2: 0 facts introduced, 0 contradictions — cumulative still 1/2 = 0.5
        {
            let acc = collector.accumulator_mut();
            acc.messages_processed = 5;
            acc.facts_introduced = 0;
            acc.contradictions_detected = 0;
        }
        collector.snapshot(utc(2026, 4, 2, 12, 0), 2, 0.8, 0.5, 0.5, 1, 0.5, 100.0);
        assert!(
            (collector.timeline[1].contradiction_detection_rate - 0.5).abs() < 1e-9,
            "epoch 2: expected 0.5 (cumulative), got {}",
            collector.timeline[1].contradiction_detection_rate
        );

        // Epoch 3: 3 facts introduced, 2 contradictions — cumulative (1+2)/(2+3) = 3/5 = 0.6
        {
            let acc = collector.accumulator_mut();
            acc.messages_processed = 5;
            acc.facts_introduced = 3;
            acc.contradictions_detected = 2;
        }
        collector.snapshot(utc(2026, 4, 3, 12, 0), 3, 0.8, 0.5, 0.5, 1, 0.5, 100.0);
        assert!(
            (collector.timeline[2].contradiction_detection_rate - 0.6).abs() < 1e-9,
            "epoch 3: expected 0.6 (cumulative 3/5), got {}",
            collector.timeline[2].contradiction_detection_rate
        );
    }
}
