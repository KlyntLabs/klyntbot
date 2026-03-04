//! Activity tracker: polls the active window every N seconds,
//! buffers events, and batch-writes to SQLite.

pub mod categorizer;
pub mod macos;

use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use chrono::Utc;
use tracing::{debug, info, warn};

use crate::config::{PrivacyConfig, ProductivityConfig};
use crate::repos::ProductivityRepos;
use crate::types::ActivityEvent;
use categorizer::Categorizer;

const MAX_BUFFER_SIZE: usize = 1000;

pub struct ActivityTracker {
    config: ProductivityConfig,
    repos: ProductivityRepos,
    categorizer: Arc<RwLock<Categorizer>>,
    cancel_token: CancellationToken,
    task_handle: Option<JoinHandle<()>>,
}

impl ActivityTracker {
    pub fn new(
        config: ProductivityConfig,
        repos: ProductivityRepos,
        categorizer: Categorizer,
    ) -> Self {
        Self {
            config,
            repos,
            categorizer: Arc::new(RwLock::new(categorizer)),
            cancel_token: CancellationToken::new(),
            task_handle: None,
        }
    }

    pub fn categorizer(&self) -> &Arc<RwLock<Categorizer>> {
        &self.categorizer
    }

    pub fn start(&mut self) {
        let cancel = self.cancel_token.clone();
        let poll_interval =
            std::time::Duration::from_secs(self.config.tracking.poll_interval_secs);
        let batch_interval =
            std::time::Duration::from_secs(self.config.tracking.batch_write_interval_secs);
        let idle_threshold = self.config.tracking.idle_threshold_secs as f64;
        let categorizer = Arc::clone(&self.categorizer);
        let privacy = self.config.privacy.clone();
        let repos = self.repos.clone();

        let handle = tokio::spawn(async move {
            let mut buffer: Vec<ActivityEvent> = Vec::new();
            let mut current_event: Option<ActivityEvent> = None;
            let mut last_flush = tokio::time::Instant::now();

            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        // Finalize current event
                        if let Some(evt) = current_event.take() {
                            buffer.push(evt);
                        }
                        // Flush remaining buffer
                        if !buffer.is_empty() {
                            if let Err(e) = repos.events.insert_batch(&buffer).await {
                                warn!("Failed to flush activity buffer on shutdown: {e}");
                            }
                        }
                        break;
                    }
                    _ = tokio::time::sleep(poll_interval) => {
                        match macos::get_frontmost_window() {
                            Ok(Some(info)) => {
                                if is_excluded(&info, &privacy) {
                                    continue;
                                }

                                let idle_secs = macos::seconds_since_last_input();
                                let is_idle = idle_secs >= idle_threshold;
                                let now = Utc::now();

                                let same_app = !is_idle
                                    && current_event.as_ref().map(|e| e.app_name.as_str())
                                        == Some(&info.app_name);

                                if same_app {
                                    if let Some(ref mut evt) = current_event {
                                        evt.ended_at = Some(now);
                                        evt.duration_secs = Some(
                                            (now - evt.started_at).num_seconds()
                                        );
                                        evt.window_title = info.window_title;
                                    }
                                } else {
                                    let cat = categorizer.read().await;
                                    let category_id = cat.categorize(
                                        &info.app_name,
                                        info.bundle_id.as_deref(),
                                        None,
                                    ).map(|c| c.id.clone());
                                    drop(cat);

                                    if let Some(evt) = current_event.take() {
                                        buffer.push(evt);
                                    }
                                    current_event = Some(ActivityEvent {
                                        id: None,
                                        app_name: info.app_name,
                                        window_title: info.window_title,
                                        bundle_id: info.bundle_id,
                                        url: None,
                                        category_id,
                                        started_at: now,
                                        ended_at: Some(now),
                                        duration_secs: Some(0),
                                        is_idle,
                                        metadata: None,
                                    });
                                }
                            }
                            Ok(None) => {
                                debug!("No frontmost window detected");
                            }
                            Err(e) => {
                                warn!("Failed to get window info: {e}");
                            }
                        }

                        // Batch write check
                        if last_flush.elapsed() >= batch_interval && !buffer.is_empty() {
                            if let Err(e) = repos.events.insert_batch(&buffer).await {
                                warn!("Failed to batch write activity events: {e}");
                            } else {
                                debug!("Flushed {} activity events", buffer.len());
                                buffer.clear();
                            }
                            last_flush = tokio::time::Instant::now();
                        }

                        // Cap buffer to prevent unbounded growth on persistent DB errors
                        if buffer.len() > MAX_BUFFER_SIZE {
                            let overflow = buffer.len() - MAX_BUFFER_SIZE;
                            warn!("Activity buffer exceeded {MAX_BUFFER_SIZE}, dropping {overflow} oldest events");
                            buffer.drain(..overflow);
                        }
                    }
                }
            }
        });

        self.task_handle = Some(handle);
        info!(
            "Activity tracker started (poll: {}s, batch: {}s)",
            self.config.tracking.poll_interval_secs, self.config.tracking.batch_write_interval_secs
        );
    }

    pub async fn stop(&mut self) {
        self.cancel_token.cancel();
        if let Some(handle) = self.task_handle.take() {
            let _ = handle.await;
        }
        info!("Activity tracker stopped");
    }
}

fn is_excluded(info: &macos::WindowInfo, privacy: &PrivacyConfig) -> bool {
    privacy.excluded_apps.iter().any(|e| {
        info.app_name.eq_ignore_ascii_case(e)
            || info
                .bundle_id
                .as_deref()
                .is_some_and(|b| b.eq_ignore_ascii_case(e))
    })
}
