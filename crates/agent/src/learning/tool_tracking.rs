//! Tool and strategy tracking — per-tool confidence thresholds and strategy
//! classification accuracy statistics.
//!
//! Merges the former `tool_confidence` and `strategy_tracker` modules.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ===========================================================================
// Per-tool confidence thresholds
// ===========================================================================

/// Per-tool confidence thresholds. Falls back to a global default
/// when no tool-specific threshold has been set.
#[derive(Debug, Clone)]
pub struct ToolConfidenceMap {
    default_threshold: f32,
    per_tool: HashMap<String, f32>,
}

impl ToolConfidenceMap {
    pub fn new(default_threshold: f32) -> Self {
        Self {
            default_threshold,
            per_tool: HashMap::new(),
        }
    }

    /// Get the confidence threshold for a specific tool.
    pub fn get_threshold(&self, tool_name: &str) -> f32 {
        self.per_tool
            .get(tool_name)
            .copied()
            .unwrap_or(self.default_threshold)
    }

    /// Set a tool-specific threshold.
    pub fn set_threshold(&mut self, tool_name: &str, threshold: f32) {
        self.per_tool
            .insert(tool_name.to_string(), threshold.clamp(0.0, 1.0));
    }

    /// Get the global default threshold.
    pub fn default_threshold(&self) -> f32 {
        self.default_threshold
    }

    /// Number of tool-specific overrides.
    pub fn overrides_count(&self) -> usize {
        self.per_tool.len()
    }

    /// All tool names with custom thresholds.
    pub fn custom_tools(&self) -> Vec<&str> {
        self.per_tool.keys().map(|s| s.as_str()).collect()
    }
}

// ===========================================================================
// Strategy classification tracking
// ===========================================================================

/// Records the outcome of a single strategy execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyRecord {
    pub timestamp: DateTime<Utc>,
    pub request_id: String,
    /// Predicted strategy name (e.g., "DirectResponse", "ToolAssisted").
    pub predicted_strategy: String,
    /// Actually used strategy (may differ if escalation occurred).
    pub actual_strategy: String,
    /// Number of times the request escalated to a different strategy.
    pub escalation_count: u32,
    /// Tool iterations actually used.
    pub iterations_used: u32,
    /// Maximum iterations allowed by the strategy.
    pub max_iterations: u32,
    pub success: bool,
    /// Optional user satisfaction score (filled later via feedback).
    pub user_satisfaction: Option<f32>,
    /// End-to-end response time in milliseconds.
    pub response_time_ms: u64,
}

/// Aggregated statistics for a strategy type.
#[derive(Debug, Clone)]
pub struct StrategyStats {
    /// Fraction of times the predicted strategy matched the actual strategy.
    pub accuracy: f32,
    /// Average number of escalations per request.
    pub avg_escalations: f32,
    /// Average tool iterations used per request.
    pub avg_iterations: f32,
    /// Total number of records analyzed.
    pub sample_count: usize,
}

/// Compute stats for a specific predicted strategy from a set of records.
pub fn compute_stats(strategy: &str, records: &[StrategyRecord]) -> StrategyStats {
    let matching: Vec<&StrategyRecord> = records
        .iter()
        .filter(|r| r.predicted_strategy == strategy)
        .collect();

    let count = matching.len();
    if count == 0 {
        return StrategyStats {
            accuracy: 0.0,
            avg_escalations: 0.0,
            avg_iterations: 0.0,
            sample_count: 0,
        };
    }

    let correct = matching
        .iter()
        .filter(|r| r.predicted_strategy == r.actual_strategy)
        .count();
    let total_escalations: u32 = matching.iter().map(|r| r.escalation_count).sum();
    let total_iterations: u32 = matching.iter().map(|r| r.iterations_used).sum();

    StrategyStats {
        accuracy: correct as f32 / count as f32,
        avg_escalations: total_escalations as f32 / count as f32,
        avg_iterations: total_iterations as f32 / count as f32,
        sample_count: count,
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── ToolConfidenceMap tests ──────────────────────────────────

    #[test]
    fn test_default_threshold_for_unknown_tool() {
        let map = ToolConfidenceMap::new(0.7);
        assert_eq!(map.get_threshold("unknown_tool"), 0.7);
    }

    #[test]
    fn test_custom_threshold_overrides_default() {
        let mut map = ToolConfidenceMap::new(0.7);
        map.set_threshold("shell", 0.9);
        assert_eq!(map.get_threshold("shell"), 0.9);
        assert_eq!(map.get_threshold("web_search"), 0.7); // still default
    }

    #[test]
    fn test_threshold_clamped_to_range() {
        let mut map = ToolConfidenceMap::new(0.7);
        map.set_threshold("risky_tool", 1.5);
        assert_eq!(map.get_threshold("risky_tool"), 1.0);
        map.set_threshold("easy_tool", -0.5);
        assert_eq!(map.get_threshold("easy_tool"), 0.0);
    }

    #[test]
    fn test_overrides_count() {
        let mut map = ToolConfidenceMap::new(0.7);
        assert_eq!(map.overrides_count(), 0);
        map.set_threshold("shell", 0.9);
        map.set_threshold("web", 0.5);
        assert_eq!(map.overrides_count(), 2);
    }

    // ── Strategy tracking tests ─────────────────────────────────

    fn make_record(
        predicted: &str,
        actual: &str,
        escalations: u32,
        iterations: u32,
    ) -> StrategyRecord {
        StrategyRecord {
            timestamp: Utc::now(),
            request_id: "test".to_string(),
            predicted_strategy: predicted.to_string(),
            actual_strategy: actual.to_string(),
            escalation_count: escalations,
            iterations_used: iterations,
            max_iterations: 10,
            success: true,
            user_satisfaction: None,
            response_time_ms: 100,
        }
    }

    #[test]
    fn test_compute_stats_accuracy() {
        let records = vec![
            make_record("DirectResponse", "DirectResponse", 0, 0),
            make_record("DirectResponse", "ToolAssisted", 1, 3),
            make_record("DirectResponse", "DirectResponse", 0, 0),
            make_record("ToolAssisted", "ToolAssisted", 0, 5),
        ];

        let stats = compute_stats("DirectResponse", &records);
        assert_eq!(stats.sample_count, 3);
        assert!((stats.accuracy - 2.0 / 3.0).abs() < 0.01);
        assert!((stats.avg_escalations - 1.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn test_compute_stats_no_matching_records() {
        let records = vec![make_record("ToolAssisted", "ToolAssisted", 0, 5)];
        let stats = compute_stats("DirectResponse", &records);
        assert_eq!(stats.sample_count, 0);
        assert_eq!(stats.accuracy, 0.0);
    }

    #[test]
    fn test_compute_stats_avg_iterations() {
        let records = vec![
            make_record("ToolAssisted", "ToolAssisted", 0, 3),
            make_record("ToolAssisted", "ToolAssisted", 0, 7),
        ];
        let stats = compute_stats("ToolAssisted", &records);
        assert_eq!(stats.sample_count, 2);
        assert!((stats.avg_iterations - 5.0).abs() < 0.01);
    }
}
