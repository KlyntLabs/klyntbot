//! Activity tracker: polls the active window every N seconds,
//! categorizes the activity, and broadcasts `ActivityTick` events
//! for independent subscribers to consume.

pub mod categorizer;
pub mod macos;

use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use chrono::Utc;
use tracing::{debug, info, warn};

use crate::config::{PrivacyConfig, ProductivityConfig};
use crate::types::ActivityTick;
use categorizer::Categorizer;

/// Extract the domain from a URL (e.g., `https://chatgpt.com/c/abc` → `chatgpt.com`).
/// Strips protocol, path, port, and `www.` prefix.
fn extract_domain(url: &str) -> Option<String> {
    let after_proto = url.find("://").map(|i| &url[i + 3..]).unwrap_or(url);
    let host = after_proto
        .find('/')
        .map(|i| &after_proto[..i])
        .unwrap_or(after_proto);
    // Strip port
    let host = host.rfind(':').map(|i| &host[..i]).unwrap_or(host);
    let host = host.strip_prefix("www.").unwrap_or(host);
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

/// Compute the display name for top-apps grouping.
///
/// Priority:
/// 1. Domain from browser URL (e.g., `chatgpt.com`) — best for grouping
/// 2. Known-site lookup from window title (e.g., `youtube.com`) — fallback
/// 3. `None` for non-browser apps (falls back to app_name in SQL)
fn compute_site_name(
    app_name: &str,
    bundle_id: Option<&str>,
    window_title: Option<&str>,
    url: Option<&str>,
) -> Option<String> {
    if !Categorizer::is_browser(app_name, bundle_id) {
        return None;
    }
    // Prefer domain from actual URL
    if let Some(domain) = url.and_then(extract_domain) {
        return Some(domain);
    }
    // Fall back to title-based extraction (known-sites lookup → last segment)
    let title = window_title?;
    if title.is_empty() {
        return None;
    }
    Some(Categorizer::extract_site_name(title))
}

pub struct ActivityTracker {
    config: ProductivityConfig,
    categorizer: Arc<RwLock<Categorizer>>,
    cancel_token: CancellationToken,
    task_handle: Option<JoinHandle<()>>,
    tick_sender: broadcast::Sender<ActivityTick>,
}

impl ActivityTracker {
    pub fn new(
        config: ProductivityConfig,
        categorizer: Categorizer,
        tick_sender: broadcast::Sender<ActivityTick>,
    ) -> Self {
        Self {
            config,
            categorizer: Arc::new(RwLock::new(categorizer)),
            cancel_token: CancellationToken::new(),
            task_handle: None,
            tick_sender,
        }
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
        let idle_threshold = self.config.tracking.idle_threshold_secs as f64;
        let categorizer = Arc::clone(&self.categorizer);
        let privacy = self.config.privacy.clone();
        let tick_sender = self.tick_sender.clone();

        let handle = tokio::spawn(async move {
            let mut prev_app: Option<String> = None;
            let mut prev_site: Option<String> = None;

            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
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

                                let raw_title = info.window_title.as_deref();

                                // Get browser URL via AppleScript (None for non-browsers).
                                // Run on blocking thread to avoid starving the Tokio runtime.
                                let url = {
                                    let app = info.app_name.clone();
                                    let bid = info.bundle_id.clone();
                                    tokio::task::spawn_blocking(move || {
                                        macos::get_browser_url(&app, bid.as_deref())
                                    })
                                    .await
                                    .unwrap_or(None)
                                };

                                // Categorize
                                let (category_id, category_type) = if is_idle {
                                    (None, None)
                                } else {
                                    let cat = categorizer.read().await;
                                    let matched = cat.categorize_full(
                                        &info.app_name,
                                        info.bundle_id.as_deref(),
                                        url.as_deref(),
                                        raw_title,
                                    );
                                    let cid = matched.map(|c| c.id.clone());
                                    let ctype = matched.map(|c| c.category_type);
                                    drop(cat);
                                    (cid, ctype)
                                };

                                let site_name = compute_site_name(
                                    &info.app_name,
                                    info.bundle_id.as_deref(),
                                    raw_title,
                                    url.as_deref(),
                                );

                                // Detect context switch
                                let is_context_switch = !is_idle && (
                                    prev_app.as_deref() != Some(&info.app_name)
                                    || prev_site != site_name
                                );

                                let tick = ActivityTick {
                                    timestamp: now,
                                    app_name: info.app_name.clone(),
                                    bundle_id: info.bundle_id.clone(),
                                    window_title: info.window_title.clone(),
                                    site_name: site_name.clone(),
                                    url,
                                    category_id,
                                    category_type,
                                    is_idle,
                                    idle_secs,
                                    is_context_switch,
                                    project_id: None,
                                };

                                // Update previous state
                                if !is_idle {
                                    prev_app = Some(info.app_name);
                                    prev_site = site_name;
                                }

                                // Broadcast — ignore error (no subscribers)
                                let _ = tick_sender.send(tick);
                            }
                            Ok(None) => {
                                debug!("No frontmost window detected");
                            }
                            Err(e) => {
                                warn!("Failed to get window info: {e}");
                            }
                        }
                    }
                }
            }
        });

        self.task_handle = Some(handle);
        info!(
            "Activity tracker started (poll: {}s, broadcast-only)",
            self.config.tracking.poll_interval_secs
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
