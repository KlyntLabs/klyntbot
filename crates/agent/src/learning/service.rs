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
        }
    }

    /// Start the background analysis loop.
    pub fn start(&mut self) {
        let store = Arc::clone(&self.outcome_store);
        let adaptive = Arc::clone(&self.adaptive);
        let threshold = self.confidence_threshold.clone();
        let interval = self.check_interval;
        let cancel = self.cancel_token.clone();
        let trigger = Arc::clone(&self.analysis_trigger);

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
                        Self::run_analysis(&store, &adaptive, &threshold).await;
                    }
                    _ = tokio::time::sleep(interval) => {
                        Self::run_analysis(&store, &adaptive, &threshold).await;
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

    /// Run analysis, update adaptive thresholds, persist state.
    async fn run_analysis(
        store: &Arc<RwLock<OutcomeStore>>,
        adaptive: &Arc<RwLock<AdaptiveThresholds>>,
        threshold: &Option<Arc<AtomicU32>>,
    ) {
        // Read outcomes (acquire then release lock quickly)
        let (outcomes, feedback) = {
            let mut store_guard = store.write().await;
            let outcomes = match store_guard.get_all_outcomes().await {
                Ok(o) => o.to_vec(),
                Err(e) => {
                    warn!("Failed to read outcomes for analysis: {}", e);
                    return;
                }
            };
            let feedback = match store_guard.get_all_feedback().await {
                Ok(f) => f.to_vec(),
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
                threshold_atomic.store(new_threshold.to_bits(), Ordering::SeqCst);
            }
        }

        if let Err(e) = adaptive_guard.save().await {
            warn!("Failed to save learning state: {}", e);
        }
    }

    /// Run analysis immediately and return the result (for CLI use).
    pub async fn analyze_now(&self) -> Result<()> {
        Self::run_analysis(
            &self.outcome_store,
            &self.adaptive,
            &self.confidence_threshold,
        )
        .await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_start_stop_lifecycle() {
        let dir = TempDir::new().unwrap();
        let store = Arc::new(RwLock::new(OutcomeStore::new(
            dir.path().join("outcomes.jsonl"),
        )));
        let adaptive = Arc::new(RwLock::new(
            AdaptiveThresholds::load(dir.path().join("state.json"), 0.7, 0.4, 0.9, 50).await,
        ));

        let mut service = LearningService::new(
            store,
            adaptive,
            None,
            StdDuration::from_secs(3600),
        );

        service.start();
        assert!(service.task_handle.is_some());

        service.stop().await;
        assert!(service.task_handle.is_none());
    }

    #[tokio::test]
    async fn test_trigger_analysis() {
        let dir = TempDir::new().unwrap();
        let store = Arc::new(RwLock::new(OutcomeStore::new(
            dir.path().join("outcomes.jsonl"),
        )));
        let adaptive = Arc::new(RwLock::new(
            AdaptiveThresholds::load(dir.path().join("state.json"), 0.7, 0.4, 0.9, 50).await,
        ));

        let mut service = LearningService::new(
            store,
            adaptive,
            None,
            StdDuration::from_secs(3600),
        );

        service.start();
        service.trigger_analysis();

        // Give the background task time to process
        tokio::time::sleep(StdDuration::from_millis(100)).await;

        service.stop().await;
    }
}
