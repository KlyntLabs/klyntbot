//! DistractionAnalyzer — tracks productive→distracting transitions,
//! records distraction patterns, and measures recovery time.

use chrono::{DateTime, Timelike, Utc};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::repos::ProductivityRepos;
use crate::types::{ActivityTick, CategoryType, DistractionPattern};

/// Minimum idle duration (seconds) to count as a "break" for `mins_since_break`.
const BREAK_IDLE_THRESHOLD_SECS: f64 = 300.0; // 5 minutes

pub struct DistractionAnalyzer {
    cancel_token: CancellationToken,
    task_handle: Option<JoinHandle<()>>,
}

impl DistractionAnalyzer {
    pub fn start(
        mut tick_rx: broadcast::Receiver<ActivityTick>,
        repos: ProductivityRepos,
        cancel: CancellationToken,
    ) -> Self {
        let cancel_clone = cancel.clone();
        let handle = tokio::spawn(async move {
            let mut last_productive_tick: Option<ActivityTick> = None;
            let mut productive_streak_start: Option<DateTime<Utc>> = None;
            let mut last_break_time: Option<DateTime<Utc>> = None;
            let mut day_start: Option<DateTime<Utc>> = None;
            let mut pending_recovery: Option<(i64, DateTime<Utc>)> = None;

            loop {
                tokio::select! {
                    _ = cancel_clone.cancelled() => break,
                    result = tick_rx.recv() => {
                        let tick = match result {
                            Ok(t) => t,
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("DistractionAnalyzer lagged, skipped {n} ticks");
                                continue;
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                        };

                        // Track day start
                        if day_start.is_none() || tick.timestamp.date_naive() != day_start.unwrap().date_naive() {
                            day_start = Some(tick.timestamp);
                            last_break_time = Some(tick.timestamp);
                        }

                        // Track idle breaks
                        if tick.is_idle && tick.idle_secs >= BREAK_IDLE_THRESHOLD_SECS {
                            last_break_time = Some(tick.timestamp);
                            continue;
                        }

                        if tick.is_idle {
                            continue;
                        }

                        let is_productive = tick.category_type == Some(CategoryType::Productive);
                        let is_distracting = tick.category_type == Some(CategoryType::Distracting);

                        // Recovery detection: was distracting, now productive
                        if let Some((pattern_id, distraction_start)) = pending_recovery {
                            if is_productive {
                                let recovery_secs = (tick.timestamp - distraction_start).num_seconds();
                                if let Err(e) = repos.distraction_patterns.update_recovery(pattern_id, recovery_secs).await {
                                    warn!("DistractionAnalyzer: failed to update recovery: {e}");
                                }
                                debug!("DistractionAnalyzer: recovery in {recovery_secs}s");
                                pending_recovery = None;
                            }
                        }

                        // Distraction transition: was productive, now distracting
                        if is_distracting && last_productive_tick.is_some() {
                            let prev = last_productive_tick.as_ref().unwrap();
                            let now = tick.timestamp;
                            let hours_active = day_start
                                .map(|ds| (now - ds).num_seconds() as f64 / 3600.0)
                                .unwrap_or(0.0);
                            let mins_since_break = last_break_time
                                .map(|bt| (now - bt).num_seconds() as f64 / 60.0)
                                .unwrap_or(0.0);
                            let preceding_duration_mins = productive_streak_start
                                .map(|ps| (now - ps).num_seconds() as f64 / 60.0);

                            let pattern = DistractionPattern {
                                id: None,
                                date: now.format("%Y-%m-%d").to_string(),
                                hour_of_day: now.hour() as i32,
                                hours_active_today: hours_active,
                                mins_since_break,
                                preceding_app: Some(prev.app_name.clone()),
                                preceding_category: prev.category_id.clone(),
                                preceding_duration_mins,
                                distraction_app: tick.app_name.clone(),
                                distraction_category: tick.category_id.clone(),
                                recovery_secs: None,
                                created_at: now,
                            };

                            match repos.distraction_patterns.insert(&pattern).await {
                                Ok(id) => {
                                    pending_recovery = Some((id, now));
                                    debug!(
                                        "DistractionAnalyzer: {} → {} (pattern #{id})",
                                        prev.app_name, tick.app_name
                                    );
                                }
                                Err(e) => {
                                    warn!("DistractionAnalyzer: failed to insert pattern: {e}");
                                }
                            }

                            productive_streak_start = None;
                            last_productive_tick = None;
                        } else if is_productive {
                            if productive_streak_start.is_none() {
                                productive_streak_start = Some(tick.timestamp);
                            }
                            last_productive_tick = Some(tick);
                        } else {
                            // Neutral — reset productive streak
                            productive_streak_start = None;
                            last_productive_tick = None;
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
                warn!("DistractionAnalyzer task panicked: {e}");
            }
        }
    }

    /// Check current fatigue signals against historical patterns.
    /// Used by NudgeService to suggest proactive breaks.
    pub async fn is_high_risk_window(
        repos: &ProductivityRepos,
        now: DateTime<Utc>,
    ) -> common::Result<bool> {
        let today = now.format("%Y-%m-%d").to_string();
        let hour = now.hour() as i32;
        let fourteen_days_ago = (now - chrono::Duration::days(14))
            .format("%Y-%m-%d")
            .to_string();
        let by_hour = repos
            .distraction_patterns
            .count_by_hour(&fourteen_days_ago, &today)
            .await?;
        let total: i64 = by_hour.iter().map(|(_, c)| c).sum();
        let hours_with_data = by_hour.len().max(1) as f64;
        let avg_per_hour = total as f64 / hours_with_data;
        let this_hour_count = by_hour
            .iter()
            .find(|(h, _)| *h == hour)
            .map(|(_, c)| *c)
            .unwrap_or(0);
        Ok(this_hour_count as f64 > avg_per_hour * 2.0)
    }
}
