//! Tray countdown — shows the next upcoming calendar event or task deadline
//! in the macOS menu bar with a live countdown (e.g. "« 24:57 · Standup").
//! Yields to the focus timer when a session is active.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use chrono::{DateTime, Local, Utc};
use tauri::{AppHandle, Manager};

use crate::app_core::AppCore;

/// Shared flag: when `true`, the focus timer owns the tray title.
pub static FOCUS_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Cached "next item" — either a calendar event or a task deadline.
struct NextItem {
    title: String,
    time: DateTime<Utc>,
}

/// Spawn the background countdown loop. Call once during app setup.
pub fn spawn(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        countdown_loop(app).await;
    });
}

/// Notify the countdown that the focus timer state changed — re-evaluate
/// the tray title immediately instead of waiting for the next poll.
pub fn notify_focus_ended(app: &AppHandle) {
    FOCUS_ACTIVE.store(false, Ordering::Relaxed);
    if let Some(tray) = app.tray_by_id("klynt-tray") {
        let _ = tray.set_title(Some(""));
    }
}

const POLL_INTERVAL_SECS: u64 = 30;

async fn countdown_loop(app: AppHandle) {
    let mut cached: Option<NextItem> = None;
    let mut poll_counter: u64 = 0;
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));

    loop {
        interval.tick().await;

        // If focus timer is active, it owns the tray title — skip
        if FOCUS_ACTIVE.load(Ordering::Relaxed) {
            cached = None;
            poll_counter = 0;
            continue;
        }

        // Re-query DB every POLL_INTERVAL_SECS or when cache is empty
        if cached.is_none() || poll_counter >= POLL_INTERVAL_SECS {
            poll_counter = 0;
            cached = query_next_item(&app).await;
        }
        poll_counter += 1;

        match &cached {
            Some(item) => {
                let now = Utc::now();
                let total_secs = item.time.signed_duration_since(now).num_seconds();

                if total_secs <= 0 {
                    // Item time has passed — clear and re-query next tick
                    set_tray_title(&app, "");
                    cached = None;
                    poll_counter = 0;
                    continue;
                }

                let hrs = total_secs / 3600;
                let mins = (total_secs % 3600) / 60;
                let secs = total_secs % 60;
                let truncated: String = item.title.chars().take(20).collect();
                let time_str = if hrs > 0 {
                    format!("{hrs}:{mins:02}:{secs:02}")
                } else {
                    format!("{mins:02}:{secs:02}")
                };
                let title = format!("« {time_str} · {truncated}");
                set_tray_title(&app, &title);
            }
            None => {
                set_tray_title(&app, "");
            }
        }
    }
}

/// Query both the next calendar event and the next task deadline, return whichever is sooner.
/// Only returns items due today — never shows countdowns for tomorrow or later.
async fn query_next_item(app: &AppHandle) -> Option<NextItem> {
    let core = app.try_state::<Arc<AppCore>>()?;
    // Use local time for "today" boundary so it matches the user's timezone
    let end_of_today = Local::now()
        .date_naive()
        .succ_opt()?
        .and_hms_opt(0, 0, 0)?
        .and_local_timezone(Local)
        .single()?
        .with_timezone(&Utc);

    let next_event = core.next_upcoming_event().await.and_then(|e| {
        let t = DateTime::parse_from_rfc3339(&e.started_at)
            .ok()?
            .with_timezone(&Utc);
        if t >= end_of_today {
            return None;
        }
        Some(NextItem {
            title: e.title,
            time: t,
        })
    });

    let next_task = core.next_upcoming_task().await.and_then(|t| {
        let due = t.due_date?;
        if due >= end_of_today {
            return None;
        }
        Some(NextItem {
            title: t.title,
            time: due,
        })
    });

    // Pick the sooner one
    match (next_event, next_task) {
        (Some(ev), Some(tk)) => {
            if ev.time <= tk.time {
                Some(ev)
            } else {
                Some(tk)
            }
        }
        (Some(ev), None) => Some(ev),
        (None, Some(tk)) => Some(tk),
        (None, None) => None,
    }
}

fn set_tray_title(app: &AppHandle, title: &str) {
    if let Some(tray) = app.tray_by_id("klynt-tray") {
        let _ = tray.set_title(Some(title));
    }
}
