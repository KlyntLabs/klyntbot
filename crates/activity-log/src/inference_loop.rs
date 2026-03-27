use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use storage::StoragePool;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::inference::ContextInferenceEngine;
use crate::work_context_repo::WorkContextRepo;

pub struct ContextInferenceLoop;

impl ContextInferenceLoop {
    pub fn start(
        engine: Arc<ContextInferenceEngine>,
        pool: StoragePool,
        interval_mins: u64,
        dormancy_days: i64,
        cancel: CancellationToken,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            // One-time startup: reclassify "general" contexts with improved heuristics
            match engine.reclassify_general_contexts().await {
                Ok(0) => {}
                Ok(n) => info!("Reclassified {n} general context(s) with improved heuristics"),
                Err(e) => warn!("Context reclassification error: {e}"),
            }

            let mut inference_interval =
                tokio::time::interval(Duration::from_secs(interval_mins * 60));
            inference_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            // Run archival check every 6 hours, deferring the first tick
            let archival_period = Duration::from_secs(6 * 60 * 60);
            let mut archival_interval =
                tokio::time::interval_at(Instant::now() + archival_period, archival_period);
            archival_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        debug!("ContextInferenceLoop shutting down");
                        break;
                    }
                    _ = inference_interval.tick() => {
                        let since = Utc::now() - chrono::Duration::minutes((interval_mins as i64) + 1);
                        match engine.process_recent_events(since).await {
                            Ok(assignments) => {
                                if !assignments.is_empty() {
                                    debug!("Assigned {} events to work contexts", assignments.len());
                                }
                            }
                            Err(e) => {
                                warn!("Context inference error: {e}");
                            }
                        }

                        // Check merge candidates after assignment
                        match engine.check_merge_candidates().await {
                            Ok(merged) if !merged.is_empty() => {
                                info!("Merged {} context pair(s)", merged.len());
                            }
                            Err(e) => {
                                warn!("Context merge check error: {e}");
                            }
                            _ => {}
                        }

                        // Record last inference run timestamp
                        if let Err(e) = WorkContextRepo::record_inference_run(&pool).await {
                            warn!("Failed to record inference run: {e}");
                        }
                    }
                    _ = archival_interval.tick() => {
                        match WorkContextRepo::archive_dormant(&pool, dormancy_days).await {
                            Ok(0) => {}
                            Ok(n) => info!("Archived {n} dormant work context(s)"),
                            Err(e) => warn!("Dormant context archival error: {e}"),
                        }
                    }
                }
            }
        })
    }
}
