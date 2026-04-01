use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::metrics::ground_truth::CheckpointResult;
use crate::metrics::{BaselineMetrics, MetricSnapshot, RegressionAlert};

// ---------------------------------------------------------------------------
// SimulationReport
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct SimulationReport {
    pub scenario: String,
    pub persona: String,
    pub simulated_days: u32,
    pub wall_time_secs: f64,
    pub seed: u64,
    pub metric_timeline: Vec<MetricSnapshot>,
    pub checkpoints: Vec<CheckpointResult>,
    pub summary: ReportSummary,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReportSummary {
    pub total_messages: u32,
    pub total_facts_extracted: u32,
    pub total_facts_superseded: u32,
    pub total_brain_versions: u32,
    pub total_autotuner_promotions: u32,
    pub total_autotuner_reverts: u32,
    pub final_metrics: MetricSnapshot,
    pub baseline_metrics: Option<BaselineMetrics>,
    pub improvement_pct: HashMap<String, f64>,
    pub checkpoint_pass_rate: f64,
    pub regression_alerts: Vec<RegressionAlert>,
}

impl SimulationReport {
    /// Write JSON report to the given directory. Returns the file path.
    ///
    /// Filename format: `{scenario}_{timestamp}.json` where timestamp is
    /// `Utc::now()` formatted as `%Y%m%d_%H%M%S`.
    pub fn write_json(&self, dir: &Path) -> common::Result<PathBuf> {
        std::fs::create_dir_all(dir)?;

        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("{}_{}.json", self.scenario, timestamp);
        let path = dir.join(filename);

        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;

        Ok(path)
    }

    /// Returns `true` if all checkpoints passed and there are no regression alerts.
    pub fn passed(&self) -> bool {
        self.summary.checkpoint_pass_rate >= 1.0 && self.summary.regression_alerts.is_empty()
    }
}

// ---------------------------------------------------------------------------
// compute_improvements
// ---------------------------------------------------------------------------

/// Compute percentage improvements from baseline to final metrics.
///
/// For most metrics, improvement = `(final - baseline) / baseline * 100`.
/// For `token_efficiency`, lower is better so improvement = `(baseline - final) / baseline * 100`.
/// Only includes metrics where the baseline is > 0.0.
pub fn compute_improvements(
    baselines: &BaselineMetrics,
    final_metrics: &MetricSnapshot,
) -> HashMap<String, f64> {
    let mut improvements = HashMap::new();

    // Higher is better: (final - baseline) / baseline * 100
    if baselines.personalization_score > 0.0 {
        let pct = (final_metrics.personalization_score - baselines.personalization_score)
            / baselines.personalization_score
            * 100.0;
        improvements.insert("personalization_score".to_string(), pct);
    }

    if baselines.routing_stability > 0.0 {
        let pct = (final_metrics.routing_stability - baselines.routing_stability)
            / baselines.routing_stability
            * 100.0;
        improvements.insert("routing_stability".to_string(), pct);
    }

    if baselines.task_completion_rate > 0.0 {
        let pct = (final_metrics.task_completion_rate - baselines.task_completion_rate)
            / baselines.task_completion_rate
            * 100.0;
        improvements.insert("task_completion_rate".to_string(), pct);
    }

    if baselines.insight_usefulness > 0.0 {
        let pct = (final_metrics.insight_usefulness - baselines.insight_usefulness)
            / baselines.insight_usefulness
            * 100.0;
        improvements.insert("insight_usefulness".to_string(), pct);
    }

    // Lower is better: (baseline - final) / baseline * 100
    if baselines.token_efficiency > 0.0 {
        let pct = (baselines.token_efficiency - final_metrics.token_efficiency)
            / baselines.token_efficiency
            * 100.0;
        improvements.insert("token_efficiency".to_string(), pct);
    }

    improvements
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::MetricSnapshot;

    fn default_summary(
        checkpoint_pass_rate: f64,
        regression_alerts: Vec<RegressionAlert>,
    ) -> ReportSummary {
        ReportSummary {
            total_messages: 100,
            total_facts_extracted: 50,
            total_facts_superseded: 5,
            total_brain_versions: 3,
            total_autotuner_promotions: 2,
            total_autotuner_reverts: 0,
            final_metrics: MetricSnapshot::default(),
            baseline_metrics: None,
            improvement_pct: HashMap::new(),
            checkpoint_pass_rate,
            regression_alerts,
        }
    }

    fn default_report(summary: ReportSummary) -> SimulationReport {
        SimulationReport {
            scenario: "test_scenario".to_string(),
            persona: "test_persona".to_string(),
            simulated_days: 90,
            wall_time_secs: 12.5,
            seed: 42,
            metric_timeline: Vec::new(),
            checkpoints: Vec::new(),
            summary,
        }
    }

    #[test]
    fn passed_with_clean_report() {
        let summary = default_summary(1.0, Vec::new());
        let report = default_report(summary);
        assert!(report.passed());
    }

    #[test]
    fn failed_with_regression() {
        let alert = RegressionAlert {
            metric: "token_efficiency".to_string(),
            baseline: 100.0,
            current: 150.0,
            regression_pct: 50.0,
        };
        let summary = default_summary(1.0, vec![alert]);
        let report = default_report(summary);
        assert!(!report.passed());
    }

    #[test]
    fn failed_with_low_checkpoint_pass_rate() {
        let summary = default_summary(0.75, Vec::new());
        let report = default_report(summary);
        assert!(!report.passed());
    }

    #[test]
    fn compute_improvements_basic() {
        let baselines = BaselineMetrics {
            token_efficiency: 500.0,
            personalization_score: 0.5,
            task_completion_rate: 0.6,
            routing_stability: 0.8,
            insight_usefulness: 0.4,
        };

        let final_metrics = MetricSnapshot {
            token_efficiency: 400.0,       // improved (lower is better)
            personalization_score: 0.75,   // improved 50%
            task_completion_rate: 0.6,     // unchanged (0%)
            routing_stability: 0.9,        // improved 12.5%
            insight_usefulness: 0.6,       // improved 50%
            ..MetricSnapshot::default()
        };

        let improvements = compute_improvements(&baselines, &final_metrics);

        // token_efficiency: (500 - 400) / 500 * 100 = 20%
        assert!((improvements["token_efficiency"] - 20.0).abs() < 1e-9);

        // personalization_score: (0.75 - 0.5) / 0.5 * 100 = 50%
        assert!((improvements["personalization_score"] - 50.0).abs() < 1e-9);

        // task_completion_rate: (0.6 - 0.6) / 0.6 * 100 = 0%
        assert!((improvements["task_completion_rate"] - 0.0).abs() < 1e-9);

        // routing_stability: (0.9 - 0.8) / 0.8 * 100 = 12.5%
        assert!((improvements["routing_stability"] - 12.5).abs() < 1e-9);

        // insight_usefulness: (0.6 - 0.4) / 0.4 * 100 = 50%
        assert!((improvements["insight_usefulness"] - 50.0).abs() < 1e-9);
    }

    #[test]
    fn compute_improvements_skips_zero_baselines() {
        let baselines = BaselineMetrics {
            token_efficiency: 0.0,
            personalization_score: 0.0,
            task_completion_rate: 0.5,
            routing_stability: 0.0,
            insight_usefulness: 0.0,
        };

        let final_metrics = MetricSnapshot {
            task_completion_rate: 0.75,
            ..MetricSnapshot::default()
        };

        let improvements = compute_improvements(&baselines, &final_metrics);

        // Only task_completion_rate should be present (the only non-zero baseline)
        assert_eq!(improvements.len(), 1);
        assert!(improvements.contains_key("task_completion_rate"));
        assert!((improvements["task_completion_rate"] - 50.0).abs() < 1e-9);
    }

    #[test]
    fn write_json_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let summary = default_summary(1.0, Vec::new());
        let report = default_report(summary);

        let path = report.write_json(dir.path()).unwrap();

        assert!(path.exists());
        assert!(path.extension().is_some_and(|ext| ext == "json"));

        let content = std::fs::read_to_string(&path).unwrap();
        let deserialized: SimulationReport = serde_json::from_str(&content).unwrap();
        assert_eq!(deserialized.scenario, "test_scenario");
        assert_eq!(deserialized.simulated_days, 90);
    }
}
