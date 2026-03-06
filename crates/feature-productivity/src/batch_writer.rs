//! BatchWriter — subscribes to ActivityTick broadcast, buffers events,
//! and batch-writes to the activity_events table.

use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::config::PrivacyConfig;
use crate::repos::ProductivityRepos;
use crate::types::{ActivityEvent, ActivityTick};

const MAX_BUFFER_SIZE: usize = 1000;

pub struct BatchWriter {
    cancel_token: CancellationToken,
    task_handle: Option<JoinHandle<()>>,
}

impl BatchWriter {
    pub fn start(
        mut tick_rx: broadcast::Receiver<ActivityTick>,
        repos: ProductivityRepos,
        privacy: PrivacyConfig,
        batch_interval_secs: u64,
        cancel: CancellationToken,
    ) -> Self {
        let cancel_clone = cancel.clone();
        let handle = tokio::spawn(async move {
            let batch_interval = std::time::Duration::from_secs(batch_interval_secs);
            let mut buffer: Vec<ActivityEvent> = Vec::new();
            let mut current_event: Option<ActivityEvent> = None;
            let mut last_flush = tokio::time::Instant::now();

            loop {
                tokio::select! {
                    _ = cancel_clone.cancelled() => {
                        if let Some(evt) = current_event.take() {
                            buffer.push(evt);
                        }
                        if !buffer.is_empty() {
                            if let Err(e) = repos.events.insert_batch(&buffer).await {
                                warn!("BatchWriter: failed to flush on shutdown: {e}");
                            }
                        }
                        break;
                    }
                    result = tick_rx.recv() => {
                        let tick = match result {
                            Ok(t) => t,
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("BatchWriter lagged, skipped {n} ticks");
                                continue;
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                        };

                        let persisted_title = if privacy.exclude_window_titles {
                            None
                        } else {
                            tick.window_title.clone()
                        };
                        let persisted_site = if privacy.exclude_window_titles {
                            None
                        } else {
                            tick.site_name.clone()
                        };

                        let same_context = !tick.is_idle
                            && !tick.is_context_switch
                            && current_event.as_ref().is_some_and(|e| {
                                e.app_name == tick.app_name && e.site_name == persisted_site
                            });

                        if same_context {
                            if let Some(ref mut evt) = current_event {
                                evt.ended_at = Some(tick.timestamp);
                                evt.duration_secs = Some(
                                    (tick.timestamp - evt.started_at).num_seconds()
                                );
                                evt.window_title = persisted_title;
                            }
                        } else {
                            if let Some(evt) = current_event.take() {
                                buffer.push(evt);
                            }
                            current_event = Some(ActivityEvent {
                                id: None,
                                app_name: tick.app_name.clone(),
                                window_title: persisted_title,
                                site_name: persisted_site,
                                bundle_id: tick.bundle_id.clone(),
                                url: None,
                                category_id: tick.category_id.clone(),
                                started_at: tick.timestamp,
                                ended_at: Some(tick.timestamp),
                                duration_secs: Some(0),
                                is_idle: tick.is_idle,
                                metadata: None,
                            });
                        }

                        // Batch write check
                        if last_flush.elapsed() >= batch_interval && !buffer.is_empty() {
                            if let Err(e) = repos.events.insert_batch(&buffer).await {
                                warn!("BatchWriter: failed to batch write: {e}");
                            } else {
                                debug!("BatchWriter: flushed {} events", buffer.len());
                                buffer.clear();
                            }
                            last_flush = tokio::time::Instant::now();
                        }

                        if buffer.len() > MAX_BUFFER_SIZE {
                            let overflow = buffer.len() - MAX_BUFFER_SIZE;
                            warn!("BatchWriter: buffer exceeded {MAX_BUFFER_SIZE}, dropping {overflow} oldest");
                            buffer.drain(..overflow);
                        }
                    }
                }
            }
        });

        Self {
            cancel_token: cancel,
            task_handle: Some(handle),
        }
    }

    pub async fn stop(&mut self) {
        self.cancel_token.cancel();
        if let Some(handle) = self.task_handle.take() {
            if let Err(e) = handle.await {
                warn!("BatchWriter task panicked: {e}");
            }
        }
    }
}
