//! LearningAnalyzer — computes statistics from outcome data.

use std::collections::HashMap;

use super::types::{AnalysisResult, ConfidenceBand, OutcomeRecord, ToolStats};

/// Confidence band boundaries for bucketing.
const BAND_BOUNDARIES: &[(f32, f32)] =
    &[(0.0, 0.3), (0.3, 0.5), (0.5, 0.7), (0.7, 0.85), (0.85, 1.0)];

pub struct LearningAnalyzer;

impl LearningAnalyzer {
    /// Analyze all outcomes and produce an AnalysisResult.
    pub fn analyze(outcomes: &[OutcomeRecord]) -> AnalysisResult {
        let per_tool_stats = Self::compute_tool_stats(outcomes);
        let (suggested_threshold, threshold_confidence) = Self::suggest_threshold(&per_tool_stats);

        AnalysisResult {
            computed_at: jiff::Timestamp::now(),
            total_outcomes: outcomes.len(),
            per_tool_stats,
            suggested_threshold,
            threshold_confidence,
        }
    }

    /// Compute per-tool stats with confidence bands.
    fn compute_tool_stats(outcomes: &[OutcomeRecord]) -> HashMap<String, ToolStats> {
        let mut by_tool: HashMap<String, Vec<&OutcomeRecord>> = HashMap::new();
        for outcome in outcomes {
            by_tool
                .entry(outcome.tool_name.clone())
                .or_default()
                .push(outcome);
        }

        by_tool
            .into_iter()
            .map(|(name, records)| {
                let total_calls = records.len();
                let success_count = records.iter().filter(|r| r.success).count();
                let total_duration: u64 = records.iter().map(|r| r.duration_ms).sum();
                let avg_duration_ms = if total_calls > 0 {
                    total_duration as f64 / total_calls as f64
                } else {
                    0.0
                };

                let bands = BAND_BOUNDARIES
                    .iter()
                    .map(|&(lower, upper)| {
                        let in_band: Vec<_> = records
                            .iter()
                            .filter(|r| {
                                r.confidence_score
                                    .map(|s| s >= lower && s < upper)
                                    .unwrap_or(false)
                            })
                            .collect();
                        let band_total = in_band.len();
                        let band_successes = in_band.iter().filter(|r| r.success).count();
                        ConfidenceBand {
                            lower,
                            upper,
                            total: band_total,
                            successes: band_successes,
                            success_rate: if band_total > 0 {
                                band_successes as f32 / band_total as f32
                            } else {
                                0.0
                            },
                        }
                    })
                    .collect();

                let stats = ToolStats {
                    total_calls,
                    success_count,
                    avg_duration_ms,
                    success_rate_by_confidence_band: bands,
                };
                (name, stats)
            })
            .collect()
    }

    /// Suggest optimal threshold based on success rate analysis.
    /// Finds the threshold where marginal success rate drops below 80%.
    fn suggest_threshold(tool_stats: &HashMap<String, ToolStats>) -> (f32, f32) {
        // Aggregate all bands across all tools
        let mut aggregate_bands: Vec<(f32, usize, usize)> = BAND_BOUNDARIES
            .iter()
            .map(|&(lower, _)| (lower, 0usize, 0usize))
            .collect();

        for stats in tool_stats.values() {
            for (i, band) in stats.success_rate_by_confidence_band.iter().enumerate() {
                if i < aggregate_bands.len() {
                    aggregate_bands[i].1 += band.total;
                    aggregate_bands[i].2 += band.successes;
                }
            }
        }

        // Find the lowest confidence band where success rate >= 80%
        let mut suggested = 0.7f32; // default
        let mut total_data_points = 0usize;

        for (lower, total, successes) in &aggregate_bands {
            total_data_points += total;
            if *total >= 5 {
                let rate = *successes as f32 / *total as f32;
                if rate >= 0.8 {
                    suggested = *lower;
                    break;
                }
            }
        }

        // Confidence in our suggestion depends on data volume
        let threshold_confidence = if total_data_points >= 100 {
            0.9
        } else if total_data_points >= 50 {
            0.7
        } else if total_data_points >= 20 {
            0.5
        } else {
            0.2
        };

        (suggested, threshold_confidence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_outcome(tool: &str, success: bool, confidence: Option<f32>) -> OutcomeRecord {
        OutcomeRecord {
            id: uuid::Uuid::new_v4().to_string(),
            session_key: "test:hash".to_string(),
            tool_name: tool.to_string(),
            success,
            error_category: None,
            duration_ms: 50,
            confidence_score: confidence,
            confidence_dimensions: None,
            created_at: jiff::Timestamp::now(),
        }
    }

    #[test]
    fn test_analyze_empty_data() {
        let result = LearningAnalyzer::analyze(&[]);
        assert_eq!(result.total_outcomes, 0);
        assert!(result.per_tool_stats.is_empty());
    }

    #[test]
    fn test_per_tool_stats() {
        let outcomes = vec![
            make_outcome("todo", true, Some(0.8)),
            make_outcome("todo", true, Some(0.9)),
            make_outcome("todo", false, Some(0.3)),
            make_outcome("shell", true, Some(0.6)),
        ];

        let result = LearningAnalyzer::analyze(&outcomes);
        assert_eq!(result.per_tool_stats.len(), 2);

        let todo_stats = &result.per_tool_stats["todo"];
        assert_eq!(todo_stats.total_calls, 3);
        assert_eq!(todo_stats.success_count, 2);
    }

    #[test]
    fn test_threshold_suggestion_defaults_on_low_data() {
        let outcomes = vec![make_outcome("todo", true, Some(0.8))];
        let result = LearningAnalyzer::analyze(&outcomes);
        // With insufficient data, threshold confidence should be low
        assert!(result.threshold_confidence <= 0.5);
    }
}
