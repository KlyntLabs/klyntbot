//! TrialPreviewSubscriber — listens for TrialActivated, starts a 4-hour timer,
//! queries early metrics via EarlyTrialEvaluator, writes preview to repo.

use bus::DomainEvent;
use dashmap::DashMap;
use jiff::Timestamp;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::super::narratives::snippet_from_alert;
use super::super::repo::MirrorRepo;
use super::super::types::*;

const PREVIEW_DELAY_SECS: u64 = 4 * 60 * 60; // 4 hours
const MIN_MESSAGES_FOR_KILL: u32 = 5;

pub struct TrialPreviewSubscriber {
    repo: MirrorRepo,
    active_timers: Arc<DashMap<String, JoinHandle<()>>>,
    evaluator: Option<Arc<dyn EarlyTrialEvaluator>>,
}

impl TrialPreviewSubscriber {
    pub fn new(
        repo: MirrorRepo,
        active_timers: Arc<DashMap<String, JoinHandle<()>>>,
        evaluator: Option<Arc<dyn EarlyTrialEvaluator>>,
    ) -> Self {
        Self {
            repo,
            active_timers,
            evaluator,
        }
    }

    // NOTE: Phase 4 ships with evaluator=None (stub). Without a real evaluator,
    // all trials produce correction_rate_delta=0.0 / Stable -> NeedMoreData recommendation.
    // The EarlyTrialEvaluator implementation requires MetricSource integration
    // which is deferred to Phase 5.

    pub async fn run(
        self: Arc<Self>,
        mut rx: broadcast::Receiver<DomainEvent>,
        shutdown: CancellationToken,
    ) {
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => break,
                event = rx.recv() => {
                    match event {
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("TrialPreviewSubscriber lagged {n} events");
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
        for entry in self.active_timers.iter() {
            entry.value().abort();
        }
        self.active_timers.clear();
    }

    fn start_preview_timer(&self, trial_id: String, _hypothesis: String) {
        let repo = self.repo.clone();
        let evaluator = self.evaluator.clone();
        let timers = self.active_timers.clone();
        let tid = trial_id.clone();

        let handle = tokio::spawn(async move {
            let started_at = Timestamp::now();
            tokio::time::sleep(std::time::Duration::from_secs(PREVIEW_DELAY_SECS)).await;

            let signals = if let Some(eval) = &evaluator {
                eval.evaluate_trial_early(&trial_id, started_at)
                    .await
                    .unwrap_or_default()
            } else {
                TrialEarlySignals::default()
            };

            let messages_scored = signals.messages_scored;
            let recommendation = compute_recommendation(&signals, messages_scored);

            let narrative = format!(
                "After 4 hours ({} messages): correction rate {:.1}% vs champion. {}.",
                messages_scored,
                signals.correction_rate_delta * 100.0,
                match &recommendation {
                    PreviewRecommendation::Continue => "Looking good — keep going",
                    PreviewRecommendation::Kill => "Trending down — consider killing early",
                    PreviewRecommendation::NeedMoreData => "Not enough data yet — keep watching",
                }
            );

            let preview = TrialPreview {
                id: Uuid::new_v4(),
                trial_id: trial_id.clone(),
                started_at,
                preview_at: Timestamp::now(),
                messages_scored,
                early_signals: signals,
                recommendation: recommendation.clone(),
                narrative: narrative.clone(),
            };

            let _ = repo.insert_trial_preview(&preview).await;

            if recommendation == PreviewRecommendation::Kill {
                let alert = MirrorAlert::TrialUnpromising {
                    trial_id: trial_id.clone(),
                    reason: narrative,
                };
                let snippet = snippet_from_alert(&alert);
                let _ = repo.insert_snippet(&snippet).await;
            }

            timers.remove(&trial_id);
        });

        // Abort any existing timer for this trial before inserting the new one
        if let Some((_, old_handle)) = self.active_timers.remove(&tid) {
            old_handle.abort();
        }
        self.active_timers.insert(tid, handle);
    }
}

/// Determine recommendation based on early signals.
pub fn compute_recommendation(
    signals: &TrialEarlySignals,
    messages_scored: u32,
) -> PreviewRecommendation {
    // Kill: correction rate worsened >10% OR insufficient data trending badly
    if signals.correction_rate_delta < -0.10 {
        return PreviewRecommendation::Kill;
    }
    if messages_scored < MIN_MESSAGES_FOR_KILL
        && signals.confidence_trend == TrendDirection::Falling
    {
        return PreviewRecommendation::Kill;
    }
    // Continue: positive delta with rising/stable confidence AND enough data
    if messages_scored >= MIN_MESSAGES_FOR_KILL
        && signals.correction_rate_delta > 0.0
        && (signals.confidence_trend == TrendDirection::Rising
            || signals.confidence_trend == TrendDirection::Stable)
    {
        return PreviewRecommendation::Continue;
    }
    // Everything else
    PreviewRecommendation::NeedMoreData
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_recommendation_kill() {
        let signals = TrialEarlySignals {
            correction_rate_delta: -0.15,
            confidence_trend: TrendDirection::Falling,
            dominant_skill_shift: None,
            messages_scored: 0,
        };
        assert_eq!(
            compute_recommendation(&signals, 20),
            PreviewRecommendation::Kill
        );
    }

    #[test]
    fn test_compute_recommendation_continue() {
        let signals = TrialEarlySignals {
            correction_rate_delta: 0.05,
            confidence_trend: TrendDirection::Rising,
            dominant_skill_shift: None,
            messages_scored: 0,
        };
        assert_eq!(
            compute_recommendation(&signals, 20),
            PreviewRecommendation::Continue
        );
    }

    #[test]
    fn test_compute_recommendation_need_more_data() {
        let signals = TrialEarlySignals {
            correction_rate_delta: 0.02,
            confidence_trend: TrendDirection::Stable,
            dominant_skill_shift: None,
            messages_scored: 0,
        };
        assert_eq!(
            compute_recommendation(&signals, 3),
            PreviewRecommendation::NeedMoreData
        );
    }
}
