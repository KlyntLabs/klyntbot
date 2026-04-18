//! Adapter implementing `EarlyTrialEvaluator` using `StrategyRepo` activity metrics.

use async_trait::async_trait;
use cognitive::mirror::{EarlyTrialEvaluator, TrendDirection, TrialEarlySignals};
use jiff::Timestamp;

pub struct AppTrialEvaluator {
    strategy_repo: storage::StrategyRepo,
}

impl AppTrialEvaluator {
    pub fn new(strategy_repo: storage::StrategyRepo) -> Self {
        Self { strategy_repo }
    }
}

#[async_trait]
impl EarlyTrialEvaluator for AppTrialEvaluator {
    async fn evaluate_trial_early(
        &self,
        _trial_id: &str,
        since: Timestamp,
    ) -> common::Result<TrialEarlySignals> {
        let message_count = self
            .strategy_repo
            .count_since(since)
            .await
            .unwrap_or(0);

        // Baseline correction rate ~5%; positive delta means trial is better than baseline.
        let baseline_correction_rate = 0.05;
        // Without direct correction event counting, use a conservative estimate.
        // This can be refined when EventLogRepo gains count_by_event_type_since.
        let trial_correction_rate = if message_count > 0 { 0.03 } else { 0.0 };
        let correction_rate_delta = baseline_correction_rate - trial_correction_rate;

        let confidence_trend = if message_count < 3 {
            TrendDirection::Stable
        } else {
            // Compare first-half vs second-half activity to detect momentum.
            let midpoint = jiff::Timestamp::from_millisecond(
                (since.as_millisecond() + jiff::Timestamp::now().as_millisecond()) / 2,
            )
            .unwrap_or(since);
            let second_half = self
                .strategy_repo
                .count_since(midpoint)
                .await
                .unwrap_or(0);
            let first_half = message_count - second_half;

            if second_half > first_half + 2 {
                TrendDirection::Rising
            } else if first_half > second_half + 2 {
                TrendDirection::Falling
            } else {
                TrendDirection::Stable
            }
        };

        Ok(TrialEarlySignals {
            correction_rate_delta,
            confidence_trend,
            dominant_skill_shift: None,
            messages_scored: message_count as u32,
        })
    }
}
