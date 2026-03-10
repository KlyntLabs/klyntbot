//! AutoFocusDetector — FSM that detects sustained productive work
//! and emits `AutoFocusEvent` events for real-time focus session creation.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::config::FocusConfig;
use crate::types::{ActivityTick, CategoryType};

/// A detected auto-focus session awaiting user confirmation (kept for backward compat).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoFocusSession {
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub dominant_app: String,
    pub dominant_category: Option<String>,
    pub productive_ratio: f64,
    pub total_secs: i64,
}

/// Real-time events emitted by the AutoFocusDetector FSM.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", tag = "event")]
pub enum AutoFocusEvent {
    /// Emitted when FSM transitions from Building → Focused (focus session starts)
    Started {
        started_at: DateTime<Utc>,
        dominant_app: String,
        dominant_category: Option<String>,
    },
    /// Emitted when cooldown grace expires (focus session ends)
    Ended {
        started_at: DateTime<Utc>,
        ended_at: DateTime<Utc>,
        dominant_app: String,
        dominant_category: Option<String>,
        productive_ratio: f64,
        total_secs: i64,
    },
}

/// FSM states for auto-focus detection.
#[derive(Debug, Clone, PartialEq, Eq)]
enum FocusState {
    Unfocused,
    Building {
        since: DateTime<Utc>,
    },
    Focused {
        since: DateTime<Utc>,
    },
    Cooldown {
        since: DateTime<Utc>,
        focus_since: DateTime<Utc>,
    },
}

/// 5-minute window statistics for deciding state transitions.
struct WindowStats {
    productive_secs: i64,
    total_secs: i64,
    context_switches: i64,
    app_counts: HashMap<String, i64>,
    category_counts: HashMap<String, i64>,
}

impl WindowStats {
    fn new() -> Self {
        Self {
            productive_secs: 0,
            total_secs: 0,
            context_switches: 0,
            app_counts: HashMap::new(),
            category_counts: HashMap::new(),
        }
    }

    fn productive_ratio(&self) -> f64 {
        if self.total_secs == 0 {
            return 0.0;
        }
        self.productive_secs as f64 / self.total_secs as f64
    }
}

pub struct AutoFocusDetector {
    cancel_token: CancellationToken,
    task_handle: Option<JoinHandle<()>>,
}

impl AutoFocusDetector {
    pub fn start(
        mut tick_rx: broadcast::Receiver<ActivityTick>,
        event_tx: mpsc::Sender<AutoFocusEvent>,
        config: FocusConfig,
        poll_interval_secs: u64,
        cancel: CancellationToken,
    ) -> Self {
        let cancel_clone = cancel.clone();
        let handle = tokio::spawn(async move {
            let mut state = FocusState::Unfocused;
            let mut window = WindowStats::new();
            let mut window_start: Option<DateTime<Utc>> = None;
            let mut consecutive_productive_windows: u32 = 0;

            // Accumulated stats for the session
            let mut session_productive_secs: i64 = 0;
            let mut session_total_secs: i64 = 0;
            let mut session_app_counts: HashMap<String, i64> = HashMap::new();
            let mut session_category_counts: HashMap<String, i64> = HashMap::new();

            let window_duration_secs = crate::types::BUCKET_DURATION_SECS;
            let productive_threshold = config.auto_detect_productive_threshold;
            let max_switches = config.auto_detect_max_switches as i64;
            let min_secs = config.auto_detect_min_mins as i64 * 60;
            let grace_secs = config.cooldown_grace_secs as i64;

            loop {
                tokio::select! {
                    _ = cancel_clone.cancelled() => break,
                    result = tick_rx.recv() => {
                        let tick = match result {
                            Ok(t) => t,
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(broadcast::error::RecvError::Closed) => break,
                        };

                        if tick.is_idle {
                            // Idle ticks may trigger cooldown
                            if let FocusState::Focused { since } = state {
                                if tick.idle_secs > 180.0 {
                                    state = FocusState::Cooldown {
                                        since: tick.timestamp,
                                        focus_since: since,
                                    };
                                    debug!("AutoFocus: Focused → Cooldown (idle >3min)");
                                }
                            }
                            continue;
                        }

                        // Initialize window start
                        if window_start.is_none() {
                            window_start = Some(tick.timestamp);
                        }

                        // Add tick to current window
                        let tick_secs = poll_interval_secs as i64;
                        window.total_secs += tick_secs;
                        if tick.category_type == Some(CategoryType::Productive) {
                            window.productive_secs += tick_secs;
                        }
                        if tick.is_context_switch {
                            window.context_switches += 1;
                        }
                        *window.app_counts.entry(tick.app_name.clone()).or_default() += tick_secs;
                        if let Some(ref cat) = tick.category_id {
                            *window.category_counts.entry(cat.clone()).or_default() += tick_secs;
                        }

                        // Check if the 5-min window is complete
                        let window_elapsed = window_start
                            .map(|ws| (tick.timestamp - ws).num_seconds())
                            .unwrap_or(0);

                        if window_elapsed < window_duration_secs {
                            continue;
                        }

                        // Window complete — evaluate
                        let is_productive_window = window.productive_ratio() >= productive_threshold
                            && window.context_switches <= max_switches;

                        match &state {
                            FocusState::Unfocused => {
                                if is_productive_window {
                                    consecutive_productive_windows += 1;
                                    if consecutive_productive_windows >= 3 {
                                        state = FocusState::Building { since: tick.timestamp - chrono::Duration::seconds(window_duration_secs * 3) };
                                        debug!("AutoFocus: Unfocused → Building (3 productive windows)");
                                    }
                                } else {
                                    consecutive_productive_windows = 0;
                                }
                            }
                            FocusState::Building { since } => {
                                let since_ts = *since;
                                let elapsed = (tick.timestamp - since_ts).num_seconds();
                                if !is_productive_window {
                                    state = FocusState::Unfocused;
                                    consecutive_productive_windows = 0;
                                    session_productive_secs = 0;
                                    session_total_secs = 0;
                                    session_app_counts.clear();
                                    session_category_counts.clear();
                                    debug!("AutoFocus: Building → Unfocused (non-productive window)");
                                } else if elapsed >= min_secs {
                                    state = FocusState::Focused { since: since_ts };
                                    debug!("AutoFocus: Building → Focused ({}min elapsed)", elapsed / 60);

                                    // Emit Started event immediately
                                    let dominant_app = crate::bucket_aggregator::dominant_key(&session_app_counts)
                                        .unwrap_or_else(|| "Unknown".to_string());
                                    let dominant_category = crate::bucket_aggregator::dominant_key(&session_category_counts);
                                    let event = AutoFocusEvent::Started {
                                        started_at: since_ts,
                                        dominant_app,
                                        dominant_category,
                                    };
                                    if event_tx.send(event).await.is_err() {
                                        warn!("AutoFocus: event receiver dropped (Started)");
                                    }
                                }
                            }
                            FocusState::Focused { since: _ } => {
                                if !is_productive_window {
                                    if let FocusState::Focused { since } = state {
                                        state = FocusState::Cooldown {
                                            since: tick.timestamp,
                                            focus_since: since,
                                        };
                                        debug!("AutoFocus: Focused → Cooldown (low productivity)");
                                    }
                                }
                            }
                            FocusState::Cooldown { since, focus_since } => {
                                let cooldown_elapsed = (tick.timestamp - *since).num_seconds();
                                if is_productive_window {
                                    let focus_since = *focus_since;
                                    state = FocusState::Focused { since: focus_since };
                                    debug!("AutoFocus: Cooldown → Focused (recovered)");
                                } else if cooldown_elapsed >= grace_secs {
                                    // Emit Ended event
                                    let dominant_app = crate::bucket_aggregator::dominant_key(&session_app_counts)
                                        .unwrap_or_else(|| "Unknown".to_string());
                                    let dominant_category = crate::bucket_aggregator::dominant_key(&session_category_counts);
                                    let productive_ratio = if session_total_secs > 0 {
                                        session_productive_secs as f64 / session_total_secs as f64
                                    } else {
                                        0.0
                                    };
                                    let total_secs = (tick.timestamp - *focus_since).num_seconds();

                                    let event = AutoFocusEvent::Ended {
                                        started_at: *focus_since,
                                        ended_at: *since, // cooldown start = focus end
                                        dominant_app,
                                        dominant_category,
                                        productive_ratio,
                                        total_secs,
                                    };

                                    if event_tx.send(event).await.is_err() {
                                        warn!("AutoFocus: event receiver dropped (Ended)");
                                    }

                                    // Reset
                                    state = FocusState::Unfocused;
                                    consecutive_productive_windows = 0;
                                    session_productive_secs = 0;
                                    session_total_secs = 0;
                                    session_app_counts.clear();
                                    session_category_counts.clear();
                                    debug!("AutoFocus: Cooldown → Ended → Unfocused");
                                }
                            }
                        }

                        // Accumulate session stats
                        if matches!(state, FocusState::Building { .. } | FocusState::Focused { .. }) {
                            session_productive_secs += window.productive_secs;
                            session_total_secs += window.total_secs;
                            for (app, secs) in &window.app_counts {
                                *session_app_counts.entry(app.clone()).or_default() += secs;
                            }
                            for (cat, secs) in &window.category_counts {
                                *session_category_counts.entry(cat.clone()).or_default() += secs;
                            }
                        }

                        // Reset window
                        window = WindowStats::new();
                        window_start = Some(tick.timestamp);
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
                warn!("AutoFocusDetector task panicked: {e}");
            }
        }
    }
}
