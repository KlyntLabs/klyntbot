//! DashboardEmitter — subscribes to ActivityTick broadcast and emits
//! Tauri-compatible event payloads via a callback. Tracks app switches,
//! computes live productivity score, and monitors focus state transitions.

use chrono::{DateTime, Utc};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::aggregator::compute_productivity_score;
use crate::types::{ActivityTick, CategoryType, DailySummary};

/// Event payloads emitted by the dashboard emitter.
/// These are plain structs — the desktop crate converts them to Tauri events.
#[derive(Debug, Clone)]
pub enum DashboardEvent {
    ActivitySwitch {
        from_app: Option<String>,
        to_app: String,
        to_site: Option<String>,
        category_type: Option<String>,
    },
    ScoreUpdated {
        score: f64,
        productive_secs: i64,
        distracting_secs: i64,
    },
    FocusStateChanged {
        state: String,
        since: String,
    },
}

/// Callback type for emitting events (injected by desktop crate).
pub type EmitFn = Box<dyn Fn(DashboardEvent) + Send + Sync + 'static>;

pub struct DashboardEmitter {
    cancel_token: CancellationToken,
    task_handle: Option<JoinHandle<()>>,
}

/// Rolling window for computing live productivity score.
struct ScoreWindow {
    productive_secs: i64,
    neutral_secs: i64,
    distracting_secs: i64,
    last_emit: DateTime<Utc>,
}

impl ScoreWindow {
    fn new() -> Self {
        Self {
            productive_secs: 0,
            neutral_secs: 0,
            distracting_secs: 0,
            last_emit: Utc::now(),
        }
    }

    fn record(&mut self, tick: &ActivityTick, interval_secs: i64) {
        if tick.is_idle {
            return;
        }
        match tick.category_type {
            Some(CategoryType::Productive) => self.productive_secs += interval_secs,
            Some(CategoryType::Distracting) => self.distracting_secs += interval_secs,
            _ => self.neutral_secs += interval_secs,
        }
    }

    fn score(&self) -> f64 {
        // Delegate to the authoritative scoring formula so live score
        // and stored daily score stay consistent.
        let stub = DailySummary {
            date: String::new(),
            total_active_secs: self.productive_secs + self.neutral_secs + self.distracting_secs,
            total_focus_secs: 0,
            total_break_secs: 0,
            total_idle_secs: 0,
            productive_secs: self.productive_secs,
            neutral_secs: self.neutral_secs,
            distracting_secs: self.distracting_secs,
            focus_sessions_count: 0,
            avg_session_quality: None,
            interruptions_count: 0,
            context_switches: 0,
            top_apps: vec![],
            top_categories: vec![],
            top_projects: vec![],
            productivity_score: None,
            ai_summary: None,
            deep_work_blocks: 0,
            deep_work_secs: 0,
            avg_recovery_secs: None,
        };
        compute_productivity_score(&stub)
    }

    fn should_emit(&self) -> bool {
        (Utc::now() - self.last_emit).num_seconds() >= 30
    }

    fn mark_emitted(&mut self) {
        self.last_emit = Utc::now();
    }
}

impl DashboardEmitter {
    pub fn start(
        mut tick_rx: broadcast::Receiver<ActivityTick>,
        emit: EmitFn,
        poll_interval_secs: u64,
        cancel: CancellationToken,
    ) -> Self {
        let cancel_child = cancel.clone();
        let interval_secs = poll_interval_secs.max(1) as i64;

        let handle = tokio::spawn(async move {
            let mut last_app: Option<String> = None;
            let mut score_window = ScoreWindow::new();

            loop {
                tokio::select! {
                    _ = cancel_child.cancelled() => break,
                    result = tick_rx.recv() => {
                        let tick = match result {
                            Ok(t) => t,
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("DashboardEmitter lagged {n} ticks");
                                continue;
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                        };

                        // Detect app switch
                        let current_app = if tick.is_idle {
                            "Idle".to_string()
                        } else {
                            tick.site_name.clone().unwrap_or_else(|| tick.app_name.clone())
                        };

                        if last_app.as_deref() != Some(&current_app) {
                            emit(DashboardEvent::ActivitySwitch {
                                from_app: last_app.take(),
                                to_app: current_app.clone(),
                                to_site: tick.site_name.clone(),
                                category_type: tick.category_type.map(|c| c.to_string()),
                            });
                        }
                        last_app = Some(current_app);

                        // Update score window
                        score_window.record(&tick, interval_secs);

                        // Emit score periodically (every ~30s)
                        if score_window.should_emit() {
                            emit(DashboardEvent::ScoreUpdated {
                                score: score_window.score(),
                                productive_secs: score_window.productive_secs,
                                distracting_secs: score_window.distracting_secs,
                            });
                            score_window.mark_emitted();
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
        if let Some(h) = self.task_handle.take() {
            let _ = h.await;
        }
    }
}
