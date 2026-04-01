pub mod behavioral;
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
    pub insight_usefulness: f64,
    // Tier 3 — system health
    pub autotuner_promotion_success: f64,
    pub community_stability: f64,
    pub brain_version_velocity: u32,
    // Performance
    pub wall_time_per_epoch_ms: f64,
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
    pub insight_usefulness: f64,
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
    pub tasks_created: u32,
    pub tasks_completed: u32,
}

// ---------------------------------------------------------------------------
// MetricCollector — accumulates per-epoch data and produces snapshots
// ---------------------------------------------------------------------------

pub struct MetricCollector {
    pub timeline: Vec<MetricSnapshot>,
    pub baselines: Option<BaselineMetrics>,
    baseline_day: u32,
    accumulator: EpochAccumulator,
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
        }
    }

    /// Mutable access to the current epoch's accumulator so callers can bump
    /// counters as events happen during the epoch.
    pub fn accumulator_mut(&mut self) -> &mut EpochAccumulator {
        &mut self.accumulator
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
        let acc = &self.accumulator;
        let msgs = acc.messages_processed.max(1) as f64;

        let correction_rate = acc.corrections as f64 / msgs;
        let fact_extraction_accuracy = if acc.facts_introduced == 0 {
            1.0
        } else {
            acc.facts_extracted as f64 / acc.facts_introduced as f64
        };
        let contradiction_detection_rate =
            acc.contradictions_detected as f64 / (acc.facts_introduced.max(1) as f64);

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
        let task_completion_rate = if acc.tasks_created == 0 {
            0.0
        } else {
            acc.tasks_completed as f64 / acc.tasks_created as f64
        };
        let routing_stability = acc.routing_matches as f64 / msgs;

        let personalization_score = behavioral::personalization_score(
            knowledge_retention,
            retrieval_precision,
            correction_rate,
        );

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
            insight_usefulness,
            autotuner_promotion_success,
            community_stability,
            brain_version_velocity,
            wall_time_per_epoch_ms: wall_time_ms,
        };

        self.timeline.push(snap);

        // Compute baselines once after the warm-up period.
        if self.baselines.is_none() && day >= self.baseline_day {
            self.compute_baselines();
        }

        // Reset accumulator for the next epoch.
        self.accumulator = EpochAccumulator::default();
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
            bl.insight_usefulness += s.insight_usefulness;
        }
        bl.token_efficiency /= n;
        bl.personalization_score /= n;
        bl.task_completion_rate /= n;
        bl.routing_stability /= n;
        bl.insight_usefulness /= n;

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

        // Other tier-2 metrics: regression = decrease.
        let checks: &[(&str, f64, f64)] = &[
            (
                "personalization_score",
                bl.personalization_score,
                latest.personalization_score,
            ),
            (
                "task_completion_rate",
                bl.task_completion_rate,
                latest.task_completion_rate,
            ),
            (
                "routing_stability",
                bl.routing_stability,
                latest.routing_stability,
            ),
            (
                "insight_usefulness",
                bl.insight_usefulness,
                latest.insight_usefulness,
            ),
        ];

        for &(name, baseline, current) in checks {
            if baseline > 0.0 {
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

        // personalization_score = 0.85 * 0.4 + 0.8 * 0.3 + (1 - 0.2) * 0.3
        //                       = 0.34 + 0.24 + 0.24 = 0.82
        assert!((snap.personalization_score - 0.82).abs() < 1e-9);

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
}
