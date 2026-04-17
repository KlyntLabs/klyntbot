//! BatchWriter — subscribes to ActivityTick broadcast, buffers events,
//! and batch-writes to the activity_events table.

use std::sync::Arc;

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

/// Convert an ActivityEvent to a WindowEventInput for unified log normalization.
fn to_window_input(evt: &ActivityEvent) -> activity_log::WindowEventInput {
    activity_log::WindowEventInput {
        app_name: evt.app_name.clone(),
        window_title: evt.window_title.clone(),
        url: evt.url.clone(),
        started_at: common::time::bridge::chrono_to_jiff(evt.started_at),
        duration_secs: evt.duration_secs,
        project_id: evt.project_id.clone(),
        is_idle: evt.is_idle,
    }
}

/// Dual-write a batch of events to the unified activity log.
async fn dual_write_to_unified(
    events: &[ActivityEvent],
    svc: &Arc<activity_log::ActivityIngestionService>,
) {
    let normalizer = activity_log::WindowEventNormalizer;
    let entries: Vec<_> = events
        .iter()
        .filter_map(|evt| {
            let input = to_window_input(evt);
            activity_log::ActivityNormalizer::normalize(&normalizer, &input)
        })
        .collect();
    if !entries.is_empty() {
        if let Err(e) = svc.ingest_batch(entries).await {
            warn!("Unified log dual-write failed: {e}");
        }
    }
}

impl BatchWriter {
    pub fn start(
        tick_rx: broadcast::Receiver<ActivityTick>,
        repos: ProductivityRepos,
        privacy: PrivacyConfig,
        batch_interval_secs: u64,
        cancel: CancellationToken,
    ) -> Self {
        Self::start_with_unified(tick_rx, repos, privacy, batch_interval_secs, cancel, None)
    }

    pub fn start_with_unified(
        mut tick_rx: broadcast::Receiver<ActivityTick>,
        repos: ProductivityRepos,
        privacy: PrivacyConfig,
        batch_interval_secs: u64,
        cancel: CancellationToken,
        unified_svc: Option<Arc<activity_log::ActivityIngestionService>>,
    ) -> Self {
        let cancel_clone = cancel.clone();
        let handle = tokio::spawn(async move {
            let batch_interval = std::time::Duration::from_secs(batch_interval_secs);
            let mut buffer: Vec<ActivityEvent> = Vec::new();
            let mut current_event: Option<ActivityEvent> = None;
            let mut last_flush = tokio::time::Instant::now();
            // Cache the active focus session ID to stamp activity events.
            // Checked on every tick; changes trigger event finalization.
            let mut cached_focus_id: Option<String> = None;

            loop {
                tokio::select! {
                    _ = cancel_clone.cancelled() => {
                        if let Some(evt) = current_event.take() {
                            buffer.push(evt);
                        }
                        if !buffer.is_empty() {
                            if let Some(ref svc) = unified_svc {
                                dual_write_to_unified(&buffer, svc).await;
                            }
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

                        // Refresh focus session state — lightweight indexed query
                        let live_focus_id = repos
                            .sessions
                            .get_active()
                            .await
                            .ok()
                            .flatten()
                            .map(|s| s.id);
                        let focus_changed = live_focus_id != cached_focus_id;
                        if focus_changed {
                            debug!(
                                "BatchWriter: focus state changed {:?} -> {:?}",
                                cached_focus_id, live_focus_id
                            );
                            cached_focus_id = live_focus_id.clone();
                        }

                        let (persisted_title, persisted_site, persisted_url) =
                            if privacy.exclude_window_titles {
                                (None, None, None)
                            } else {
                                (tick.window_title.clone(), tick.site_name.clone(), tick.url.clone())
                            };

                        let same_context = !tick.is_idle
                            && !tick.is_context_switch
                            && !focus_changed
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
                                url: persisted_url,
                                category_id: tick.category_id.clone(),
                                started_at: tick.timestamp,
                                ended_at: Some(tick.timestamp),
                                duration_secs: Some(0),
                                is_idle: tick.is_idle,
                                metadata: None,
                                project_id: tick.project_id.clone(),
                                focus_session_id: cached_focus_id.clone(),
                            });
                        }

                        // Batch write check
                        if last_flush.elapsed() >= batch_interval && !buffer.is_empty() {
                            if let Some(ref svc) = unified_svc {
                                dual_write_to_unified(&buffer, svc).await;
                            }
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
