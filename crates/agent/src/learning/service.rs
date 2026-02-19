//! LearningService — background analysis service.
//!
//! Follows the `RecurringTaskSpawner` pattern: `CancellationToken` +
//! `JoinHandle` + `tokio::select!` loop.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration as StdDuration;

use tokio::sync::{Notify, RwLock};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use common::Result;

use super::adaptive::AdaptiveThresholds;
use super::analyzer::LearningAnalyzer;
use super::outcome_store::OutcomeStore;

/// Background service that periodically analyzes outcomes and adapts
/// the confidence threshold.
pub struct LearningService {
    outcome_store: Arc<RwLock<OutcomeStore>>,
    adaptive: Arc<RwLock<AdaptiveThresholds>>,
    confidence_threshold: Option<Arc<AtomicU32>>,
    check_interval: StdDuration,
    task_handle: Option<JoinHandle<()>>,
    cancel_token: CancellationToken,
    analysis_trigger: Arc<Notify>,
    event_bus: Option<Arc<bus::LearningEventBus>>,
}

impl LearningService {
    pub fn new(
        outcome_store: Arc<RwLock<OutcomeStore>>,
        adaptive: Arc<RwLock<AdaptiveThresholds>>,
        confidence_threshold: Option<Arc<AtomicU32>>,
        check_interval: StdDuration,
    ) -> Self {
        Self {
            outcome_store,
            adaptive,
            confidence_threshold,
            check_interval,
            task_handle: None,
            cancel_token: CancellationToken::new(),
            analysis_trigger: Arc::new(Notify::new()),
            event_bus: None,
        }
    }

    /// Attach a `LearningEventBus` so the service publishes `AnalysisCompleted`
    /// (and `ThresholdChanged`) events after each analysis cycle.
    pub fn with_event_bus(mut self, bus: Arc<bus::LearningEventBus>) -> Self {
        self.event_bus = Some(bus);
        self
    }

    /// Start the background analysis loop.
    pub fn start(&mut self) {
        let store = Arc::clone(&self.outcome_store);
        let adaptive = Arc::clone(&self.adaptive);
        let threshold = self.confidence_threshold.clone();
        let interval = self.check_interval;
        let cancel = self.cancel_token.clone();
        let trigger = Arc::clone(&self.analysis_trigger);
        let event_bus = self.event_bus.clone();

        let handle = tokio::spawn(async move {
            info!(
                "Learning service started (interval: {}s)",
                interval.as_secs()
            );

            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        info!("Learning service shutting down");
                        break;
                    }
                    _ = trigger.notified() => {
                        info!("Learning analysis triggered manually");
                        Self::run_analysis(&store, &adaptive, &threshold, event_bus.as_deref()).await;
                    }
                    _ = tokio::time::sleep(interval) => {
                        Self::run_analysis(&store, &adaptive, &threshold, event_bus.as_deref()).await;
                    }
                }
            }
        });

        self.task_handle = Some(handle);
    }

    /// Stop the background service.
    pub async fn stop(&mut self) {
        self.cancel_token.cancel();
        if let Some(handle) = self.task_handle.take() {
            let _ = handle.await;
        }
    }

    /// Trigger immediate analysis (cancels any sleeping background timer).
    pub fn trigger_analysis(&self) {
        self.analysis_trigger.notify_one();
    }

    /// Run analysis, update adaptive thresholds, persist state, and publish events.
    async fn run_analysis(
        store: &Arc<RwLock<OutcomeStore>>,
        adaptive: &Arc<RwLock<AdaptiveThresholds>>,
        threshold: &Option<Arc<AtomicU32>>,
        event_bus: Option<&bus::LearningEventBus>,
    ) {
        // Read outcomes (acquire then release lock quickly)
        let (outcomes, feedback) = {
            let store_guard = store.read().await;
            let outcomes = match store_guard.get_all_outcomes().await {
                Ok(o) => o,
                Err(e) => {
                    warn!("Failed to read outcomes for analysis: {}", e);
                    return;
                }
            };
            let feedback = match store_guard.get_all_feedback().await {
                Ok(f) => f,
                Err(e) => {
                    warn!("Failed to read feedback for analysis: {}", e);
                    return;
                }
            };
            (outcomes, feedback)
        };

        // Analyze (no locks held)
        let analysis = LearningAnalyzer::analyze(&outcomes, &feedback);

        info!(
            "Learning analysis complete: {} outcomes, suggested threshold {:.3}",
            analysis.total_outcomes, analysis.suggested_threshold
        );

        // Apply and persist
        let mut adaptive_guard = adaptive.write().await;
        if let Some(new_threshold) = adaptive_guard.apply_analysis(&analysis) {
            if let Some(threshold_atomic) = threshold {
                let old_bits = threshold_atomic.load(Ordering::SeqCst);
                let old_threshold = f32::from_bits(old_bits);
                threshold_atomic.store(new_threshold.to_bits(), Ordering::SeqCst);

                if let Some(bus) = event_bus {
                    bus.publish(bus::LearningEvent::ThresholdChanged {
                        old_threshold,
                        new_threshold,
                        reason: "adaptive_analysis".to_string(),
                    })
                    .await;
                }
            }
        }

        if let Err(e) = adaptive_guard.save().await {
            warn!("Failed to save learning state: {}", e);
        }

        // Publish AnalysisCompleted regardless of whether threshold changed
        if let Some(bus) = event_bus {
            bus.publish(bus::LearningEvent::AnalysisCompleted {
                total_outcomes: analysis.total_outcomes,
                suggested_threshold: analysis.suggested_threshold,
            })
            .await;
        }
    }

    /// Run analysis immediately and return the result (for CLI use).
    pub async fn analyze_now(&self) -> Result<()> {
        Self::run_analysis(
            &self.outcome_store,
            &self.adaptive,
            &self.confidence_threshold,
            self.event_bus.as_deref(),
        )
        .await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bus::LearningEvent;

    fn make_test_stores() -> (Arc<RwLock<OutcomeStore>>, Arc<RwLock<AdaptiveThresholds>>) {
        let store = Arc::new(RwLock::new(OutcomeStore::new_in_memory()));
        let adaptive = Arc::new(RwLock::new(AdaptiveThresholds::new_in_memory(
            0.7, 0.4, 0.9, 50,
        )));
        (store, adaptive)
    }

    /// AC-I2.1/2.2: LearningService publishes AnalysisCompleted event when triggered.
    #[tokio::test]
    async fn test_publishes_analysis_completed_on_trigger() {
        let (store, adaptive) = make_test_stores();

        let event_bus = Arc::new(bus::LearningEventBus::new(16));
        let mut rx = event_bus.subscribe();

        let mut service = LearningService::new(store, adaptive, None, StdDuration::from_secs(3600))
            .with_event_bus(Arc::clone(&event_bus));

        service.start();
        service.trigger_analysis();

        let event = tokio::time::timeout(StdDuration::from_millis(500), rx.recv())
            .await
            .expect("timeout waiting for AnalysisCompleted event")
            .expect("channel closed");

        assert!(
            matches!(event, LearningEvent::AnalysisCompleted { .. }),
            "Expected AnalysisCompleted, got {:?}",
            event
        );

        service.stop().await;
    }

    #[tokio::test]
    async fn test_start_stop_lifecycle() {
        let (store, adaptive) = make_test_stores();

        let mut service = LearningService::new(store, adaptive, None, StdDuration::from_secs(3600));

        service.start();
        assert!(service.task_handle.is_some());

        service.stop().await;
        assert!(service.task_handle.is_none());
    }

    #[tokio::test]
    async fn test_trigger_analysis() {
        let (store, adaptive) = make_test_stores();

        let mut service = LearningService::new(store, adaptive, None, StdDuration::from_secs(3600));

        service.start();
        service.trigger_analysis();

        // Give the background task time to process
        tokio::time::sleep(StdDuration::from_millis(100)).await;

        service.stop().await;
    }
}
