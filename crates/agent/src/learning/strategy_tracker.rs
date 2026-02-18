//! Strategy classification tracking — computes accuracy and iteration stats.

use super::strategy_store::StrategyRecord;

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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_record(predicted: &str, actual: &str, escalations: u32, iterations: u32) -> StrategyRecord {
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
            make_record("DirectResponse", "ToolAssisted", 1, 3), // escalated
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
