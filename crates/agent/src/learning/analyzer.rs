//! LearningAnalyzer — computes statistics from outcome data.

use std::collections::HashMap;

use chrono::Utc;

use super::types::{
    AnalysisResult, ConfidenceBand, EnrichmentFeedbackEntry, EnrichmentStats, FieldAcceptanceStats,
    OutcomeRecord, ToolStats,
};

/// Confidence band boundaries for bucketing.
const BAND_BOUNDARIES: &[(f32, f32)] =
    &[(0.0, 0.3), (0.3, 0.5), (0.5, 0.7), (0.7, 0.85), (0.85, 1.0)];

pub struct LearningAnalyzer;

impl LearningAnalyzer {
    /// Analyze all outcomes and produce an AnalysisResult.
    pub fn analyze(
        outcomes: &[OutcomeRecord],
        feedback: &[EnrichmentFeedbackEntry],
    ) -> AnalysisResult {
        let per_tool_stats = Self::compute_tool_stats(outcomes);
        let (suggested_threshold, threshold_confidence) = Self::suggest_threshold(&per_tool_stats);
        let enrichment_stats = Self::compute_enrichment_stats(feedback);

        AnalysisResult {
            computed_at: Utc::now(),
            total_outcomes: outcomes.len(),
            per_tool_stats,
            suggested_threshold,
            threshold_confidence,
            enrichment_stats,
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

    /// Compute enrichment acceptance stats.
    fn compute_enrichment_stats(feedback: &[EnrichmentFeedbackEntry]) -> EnrichmentStats {
        if feedback.is_empty() {
            return EnrichmentStats::default();
        }

        let total = feedback.len();
        let accepted = feedback.iter().filter(|f| f.accepted).count();
        let overridden = total - accepted;

        let mut per_field: HashMap<String, (usize, usize)> = HashMap::new();
        for entry in feedback {
            let (field_total, field_accepted) = per_field.entry(entry.field.clone()).or_default();
            *field_total += 1;
            if entry.accepted {
                *field_accepted += 1;
            }
        }

        let per_field_stats = per_field
            .into_iter()
            .map(|(field, (t, a))| {
                (
                    field,
                    FieldAcceptanceStats {
                        total: t,
                        accepted: a,
                        acceptance_rate: a as f32 / t as f32,
                    },
                )
            })
            .collect();

        EnrichmentStats {
            total_suggestions: total,
            accepted_count: accepted,
            overridden_count: overridden,
            acceptance_rate: accepted as f32 / total as f32,
            per_field: per_field_stats,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::learning::types::ExecutionMode;
    use chrono::Utc;

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
            execution_mode: ExecutionMode::Chat,
            created_at: Utc::now(),
        }
    }

    fn make_feedback(field: &str, accepted: bool) -> EnrichmentFeedbackEntry {
        EnrichmentFeedbackEntry {
            task_id: "todo-1".to_string(),
            field: field.to_string(),
            suggested_value: "1".to_string(),
            actual_value: if accepted {
                None
            } else {
                Some("2".to_string())
            },
            accepted,
            confidence: 0.8,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_analyze_empty_data() {
        let result = LearningAnalyzer::analyze(&[], &[]);
        assert_eq!(result.total_outcomes, 0);
        assert!(result.per_tool_stats.is_empty());
        assert_eq!(result.enrichment_stats.total_suggestions, 0);
    }

    #[test]
    fn test_per_tool_stats() {
        let outcomes = vec![
            make_outcome("todo", true, Some(0.8)),
            make_outcome("todo", true, Some(0.9)),
            make_outcome("todo", false, Some(0.3)),
            make_outcome("shell", true, Some(0.6)),
        ];

        let result = LearningAnalyzer::analyze(&outcomes, &[]);
        assert_eq!(result.per_tool_stats.len(), 2);

        let todo_stats = &result.per_tool_stats["todo"];
        assert_eq!(todo_stats.total_calls, 3);
        assert_eq!(todo_stats.success_count, 2);
    }

    #[test]
    fn test_enrichment_stats() {
        let feedback = vec![
            make_feedback("priority", true),
            make_feedback("priority", true),
            make_feedback("priority", false),
            make_feedback("estimated_minutes", true),
        ];

        let result = LearningAnalyzer::analyze(&[], &feedback);
        assert_eq!(result.enrichment_stats.total_suggestions, 4);
        assert_eq!(result.enrichment_stats.accepted_count, 3);
        assert_eq!(result.enrichment_stats.overridden_count, 1);
        assert!((result.enrichment_stats.acceptance_rate - 0.75).abs() < f32::EPSILON);

        let priority = &result.enrichment_stats.per_field["priority"];
        assert_eq!(priority.total, 3);
        assert_eq!(priority.accepted, 2);
    }

    #[test]
    fn test_threshold_suggestion_defaults_on_low_data() {
        let outcomes = vec![make_outcome("todo", true, Some(0.8))];
        let result = LearningAnalyzer::analyze(&outcomes, &[]);
        // With insufficient data, threshold confidence should be low
        assert!(result.threshold_confidence <= 0.5);
    }
}
