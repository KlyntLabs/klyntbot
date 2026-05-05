//! TrialPreviewSource — listens for AutotunerDecision(activated), starts a 4-hour timer,
//! queries early metrics via EarlyTrialEvaluator, writes preview to repo.

use std::sync::Arc;

use ai_core::{AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use dashmap::DashMap;
use jiff::Timestamp;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::mirror::{
    EarlyTrialEvaluator, MirrorAlert, MirrorRepo, PreviewRecommendation,
    TrialEarlySignals, TrialPreview,
};

const PREVIEW_DELAY_SECS: u64 = 4 * 60 * 60;
const MIN_MESSAGES_FOR_KILL: u32 = 5;

pub struct TrialPreviewSource {
    repo: MirrorRepo,
    active_timers: Arc<DashMap<String, JoinHandle<()>>>,
    evaluator: Option<Arc<dyn EarlyTrialEvaluator>>,
}

impl TrialPreviewSource {
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
                let _ = repo.insert_snippet_from_alert(&alert).await;
            }

            timers.remove(&trial_id);
        });

        if let Some((_, old_handle)) = self.active_timers.remove(&tid) {
            old_handle.abort();
        }
        self.active_timers.insert(tid, handle);
    }
}

#[async_trait]
impl MirrorSignalSource for TrialPreviewSource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "trial_preview",
            subscribed_kinds: &["AutotunerDecision"],
            flush_interval_secs: None,
        }
    }

    fn name(&self) -> &'static str {
        "trial-preview-source"
    }

    async fn accumulate(&self, signal: &AiSignal) -> common::Result<()> {
        if let Some(bus::DomainEvent::AutotunerDecision {
            trial_id, verdict, ..
        }) = &signal.raw_event
        {
            if verdict == "activated" {
                self.start_preview_timer(trial_id.clone(), String::new());
            }
        }
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        // Event-driven only.
        Ok(())
    }
}

pub fn compute_recommendation(
    signals: &TrialEarlySignals,
    messages_scored: u32,
) -> PreviewRecommendation {
    if signals.correction_rate_delta < -0.10 {
        return PreviewRecommendation::Kill;
    }
    if messages_scored < MIN_MESSAGES_FOR_KILL
        && signals.confidence_trend == crate::mirror::TrendDirection::Falling
    {
        return PreviewRecommendation::Kill;
    }
    if messages_scored >= MIN_MESSAGES_FOR_KILL
        && signals.correction_rate_delta > 0.0
        && (signals.confidence_trend == crate::mirror::TrendDirection::Rising
            || signals.confidence_trend == crate::mirror::TrendDirection::Stable)
    {
        return PreviewRecommendation::Continue;
    }
    PreviewRecommendation::NeedMoreData
}
