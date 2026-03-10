use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::inference::ContextInferenceEngine;

pub struct ContextInferenceLoop;

impl ContextInferenceLoop {
    pub fn start(
        engine: Arc<ContextInferenceEngine>,
        interval_mins: u64,
        cancel: CancellationToken,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_mins * 60));
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        debug!("ContextInferenceLoop shutting down");
                        break;
                    }
                    _ = interval.tick() => {
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
                    }
                }
            }
        })
    }
}
