//! Activity tracker: polls the active window every N seconds,
//! buffers events, and batch-writes to SQLite.

pub mod categorizer;
pub mod macos;

use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use chrono::{DateTime, Utc};
use tracing::{debug, info, warn};

use crate::config::{PrivacyConfig, ProductivityConfig};
use crate::focus::FocusManager;
use crate::repos::ProductivityRepos;
use crate::types::{ActivityEvent, CategoryType};
use categorizer::Categorizer;

const MAX_BUFFER_SIZE: usize = 1000;
/// Minimum seconds between distraction alerts for the same app.
const DISTRACTION_COOLDOWN_SECS: i64 = 60;

/// Alert emitted when a distracting app is used during a focus session.
#[derive(Debug, Clone)]
pub struct DistractionAlert {
    pub app_name: String,
    pub session_id: String,
    pub timestamp: DateTime<Utc>,
}

pub struct ActivityTracker {
    config: ProductivityConfig,
    repos: ProductivityRepos,
    categorizer: Arc<RwLock<Categorizer>>,
    cancel_token: CancellationToken,
    task_handle: Option<JoinHandle<()>>,
    focus_manager: Option<Arc<FocusManager>>,
    distraction_sender: Option<mpsc::Sender<DistractionAlert>>,
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
            focus_manager: None,
            distraction_sender: None,
        }
    }

    /// Set focus manager for distraction detection during focus sessions.
    pub fn set_focus_manager(&mut self, mgr: Arc<FocusManager>) {
        self.focus_manager = Some(mgr);
    }

    /// Set channel for emitting distraction alerts.
    pub fn set_distraction_sender(&mut self, sender: mpsc::Sender<DistractionAlert>) {
        self.distraction_sender = Some(sender);
    }

    pub fn categorizer(&self) -> &Arc<RwLock<Categorizer>> {
        &self.categorizer
    }

    pub fn start(&mut self) {
        if self.task_handle.is_some() {
            warn!("ActivityTracker already running — ignoring duplicate start");
            return;
        }
        let cancel = self.cancel_token.clone();
        let poll_interval =
            std::time::Duration::from_secs(self.config.tracking.poll_interval_secs.max(1));
        let batch_interval =
            std::time::Duration::from_secs(self.config.tracking.batch_write_interval_secs);
        let idle_threshold = self.config.tracking.idle_threshold_secs as f64;
        let categorizer = Arc::clone(&self.categorizer);
        let privacy = self.config.privacy.clone();
        let repos = self.repos.clone();
        let focus_manager = self.focus_manager.clone();
        let distraction_sender = self.distraction_sender.clone();

        let handle = tokio::spawn(async move {
            let mut buffer: Vec<ActivityEvent> = Vec::new();
            let mut current_event: Option<ActivityEvent> = None;
            let mut last_flush = tokio::time::Instant::now();
            // Distraction cooldown tracking
            let mut last_distraction_app: Option<String> = None;
            let mut last_distraction_time: Option<DateTime<Utc>> = None;

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

                                // Strip window title if privacy config says so.
                                // We still pass the raw title to the categorizer so
                                // browser-tab-based detection works, but we never
                                // persist it in the ActivityEvent.
                                let raw_title = info.window_title.as_deref();
                                let persisted_title = if privacy.exclude_window_titles {
                                    None
                                } else {
                                    info.window_title.clone()
                                };

                                // Categorize using app name, bundle ID, AND window title.
                                // Runs every tick so browsers get re-evaluated when
                                // the user navigates to a different site (title changes).
                                // Skipped when idle — no categorization needed.
                                let (category_id, is_distracting) = if is_idle {
                                    (None, false)
                                } else {
                                    let cat = categorizer.read().await;
                                    let matched = cat.categorize_full(
                                        &info.app_name,
                                        info.bundle_id.as_deref(),
                                        None,
                                        raw_title,
                                    );
                                    let cid = matched.map(|c| c.id.clone());
                                    let distracting = matched
                                        .is_some_and(|c| c.category_type == CategoryType::Distracting);
                                    drop(cat);
                                    (cid, distracting)
                                };

                                // Distraction detection — runs every tick, not just on
                                // app switch. This catches in-browser navigation (e.g.
                                // Chrome → YouTube) where app_name stays the same but
                                // the window title changes to a distracting site.
                                if is_distracting {
                                    if let Some(ref fm) = focus_manager {
                                        let distraction_key = info
                                            .window_title
                                            .as_deref()
                                            .unwrap_or(&info.app_name)
                                            .to_string();

                                        let should_alert = !matches!(
                                            (&last_distraction_app, last_distraction_time),
                                            (Some(prev_key), Some(prev_time))
                                                if prev_key == &distraction_key
                                                    && (now - prev_time).num_seconds() < DISTRACTION_COOLDOWN_SECS
                                        );

                                        if should_alert {
                                            match fm.get_active().await {
                                                Ok(Some(session)) => {
                                                    let session_id = session.id.clone();
                                                    // Record distraction using the already-fetched session.
                                                    if let Err(e) = fm.record_distraction_for(session, &info.app_name).await {
                                                        warn!("Failed to record distraction: {e}");
                                                    }
                                                    if let Some(ref sender) = distraction_sender {
                                                        if let Err(e) = sender.try_send(DistractionAlert {
                                                            app_name: info.app_name.clone(),
                                                            session_id,
                                                            timestamp: now,
                                                        }) {
                                                            warn!("Failed to send distraction alert: {e}");
                                                        }
                                                    }
                                                    last_distraction_app = Some(distraction_key);
                                                    last_distraction_time = Some(now);
                                                }
                                                Ok(None) => {}
                                                Err(e) => {
                                                    debug!("Failed to check active session: {e}");
                                                }
                                            }
                                        }
                                    }
                                }

                                let same_app = !is_idle
                                    && current_event.as_ref().map(|e| e.app_name.as_str())
                                        == Some(&info.app_name);

                                if same_app {
                                    if let Some(ref mut evt) = current_event {
                                        evt.ended_at = Some(now);
                                        evt.duration_secs = Some(
                                            (now - evt.started_at).num_seconds()
                                        );
                                        evt.window_title = persisted_title.clone();
                                    }
                                } else {
                                    if let Some(evt) = current_event.take() {
                                        buffer.push(evt);
                                    }
                                    current_event = Some(ActivityEvent {
                                        id: None,
                                        app_name: info.app_name,
                                        window_title: persisted_title,
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
            if let Err(e) = handle.await {
                warn!("ActivityTracker task panicked: {e}");
            }
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
