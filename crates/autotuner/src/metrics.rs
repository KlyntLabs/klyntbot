use uuid::Uuid;

use crate::traits::MetricSnapshot;
use crate::trial::TrialResult;

/// Aggregates `MetricSnapshot`s into a single `TrialResult` using
/// volume-weighted averaging.  Each snapshot is weighted by its fraction
/// of the total message count so that high-traffic periods dominate the
/// aggregate rather than being averaged equally with low-traffic windows.
pub fn aggregate_to_result(trial_id: Uuid, snapshots: &[MetricSnapshot]) -> TrialResult {
    if snapshots.is_empty() {
        return TrialResult {
            trial_id,
            ..Default::default()
        };
    }

    let total_messages: u32 = snapshots.iter().map(|s| s.total_messages).sum();
    if total_messages == 0 {
        return TrialResult {
            trial_id,
            ..Default::default()
        };
    }

    // Weight each snapshot by its message volume fraction.
    let w = |s: &MetricSnapshot| s.total_messages as f64 / total_messages as f64;

    TrialResult {
        trial_id,
        messages_scored: total_messages,
        correction_rate: snapshots.iter().map(|s| s.correction_rate * w(s)).sum(),
        classification_accuracy: snapshots
            .iter()
            .map(|s| s.classification_accuracy * w(s))
            .sum(),
        avg_tokens_per_message: snapshots
            .iter()
            .map(|s| s.avg_tokens_per_message * w(s))
            .sum(),
        avg_response_time_ms: snapshots
            .iter()
            .map(|s| s.avg_response_time_ms * w(s))
            .sum(),
        routing_stability: snapshots.iter().map(|s| s.routing_stability * w(s)).sum(),
        memory_relevance: snapshots.iter().map(|s| s.memory_relevance * w(s)).sum(),
        retrieval_precision: snapshots.iter().map(|s| s.retrieval_precision * w(s)).sum(),
        retrieval_recall: snapshots.iter().map(|s| s.retrieval_recall * w(s)).sum(),
        memory_freshness: snapshots.iter().map(|s| s.memory_freshness * w(s)).sum(),
        promotion_accuracy: snapshots.iter().map(|s| s.promotion_accuracy * w(s)).sum(),
        knowledge_retention_score: snapshots
            .iter()
            .map(|s| s.knowledge_retention_score * w(s))
            .sum(),
        user_satisfaction: {
            let sats: Vec<(f64, f64)> = snapshots
                .iter()
                .filter_map(|s| s.user_satisfaction.map(|v| (v, w(s))))
                .collect();
            if sats.is_empty() {
                None
            } else {
                Some(sats.iter().map(|(v, weight)| v * weight).sum())
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_averages_by_volume() {
        let small = MetricSnapshot {
            correction_rate: 0.50,
            classification_accuracy: 0.60,
            avg_tokens_per_message: 1000.0,
            avg_response_time_ms: 2000.0,
            routing_stability: 0.50,
            memory_relevance: 0.50,
            user_satisfaction: Some(0.30),
            total_messages: 10,
            ..Default::default()
        };
        let large = MetricSnapshot {
            correction_rate: 0.10,
            classification_accuracy: 0.90,
            avg_tokens_per_message: 400.0,
            avg_response_time_ms: 600.0,
            routing_stability: 0.95,
            memory_relevance: 0.85,
            user_satisfaction: Some(0.80),
            total_messages: 90,
            ..Default::default()
        };

        let result = aggregate_to_result(Uuid::nil(), &[small, large]);

        assert_eq!(result.messages_scored, 100);

        // Expected: 0.50 * 0.10 + 0.10 * 0.90 = 0.05 + 0.09 = 0.14
        assert!((result.correction_rate - 0.14).abs() < 1e-9);
        // Expected: 0.60 * 0.10 + 0.90 * 0.90 = 0.06 + 0.81 = 0.87
        assert!((result.classification_accuracy - 0.87).abs() < 1e-9);
        // Expected: 1000 * 0.10 + 400 * 0.90 = 100 + 360 = 460
        assert!((result.avg_tokens_per_message - 460.0).abs() < 1e-9);
        // Expected: 2000 * 0.10 + 600 * 0.90 = 200 + 540 = 740
        assert!((result.avg_response_time_ms - 740.0).abs() < 1e-9);
        // Expected: 0.50 * 0.10 + 0.95 * 0.90 = 0.05 + 0.855 = 0.905
        assert!((result.routing_stability - 0.905).abs() < 1e-9);
        // Expected: 0.50 * 0.10 + 0.85 * 0.90 = 0.05 + 0.765 = 0.815
        assert!((result.memory_relevance - 0.815).abs() < 1e-9);
        // Expected: 0.30 * 0.10 + 0.80 * 0.90 = 0.03 + 0.72 = 0.75
        let sat = result.user_satisfaction.unwrap();
        assert!((sat - 0.75).abs() < 1e-9);

        // The 90-message snapshot dominates: all results are much closer to
        // its values than a simple average would produce.
        assert!(
            result.correction_rate < 0.20,
            "volume-weighted result should be close to the 90-msg snapshot (0.10), not the midpoint"
        );
    }

    #[test]
    fn aggregate_empty_returns_default() {
        let result = aggregate_to_result(Uuid::nil(), &[]);
        assert_eq!(result.messages_scored, 0);
        assert_eq!(result.correction_rate, 0.0);
        assert!(result.user_satisfaction.is_none());
    }

    #[test]
    fn aggregate_zero_messages_returns_default() {
        let zero = MetricSnapshot {
            total_messages: 0,
            ..Default::default()
        };
        let result = aggregate_to_result(Uuid::nil(), &[zero]);
        assert_eq!(result.messages_scored, 0);
    }

    #[test]
    fn aggregate_single_snapshot() {
        let snap = MetricSnapshot {
            correction_rate: 0.15,
            classification_accuracy: 0.88,
            avg_tokens_per_message: 500.0,
            avg_response_time_ms: 800.0,
            routing_stability: 0.92,
            memory_relevance: 0.80,
            user_satisfaction: None,
            total_messages: 42,
            ..Default::default()
        };
        let result = aggregate_to_result(Uuid::nil(), &[snap]);
        assert_eq!(result.messages_scored, 42);
        assert!((result.correction_rate - 0.15).abs() < 1e-9);
        assert!(result.user_satisfaction.is_none());
    }
}
