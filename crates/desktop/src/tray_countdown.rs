//! Tray countdown — shows the next upcoming calendar event or task deadline
//! in the macOS menu bar with a live countdown (e.g. "« 24:57 · Standup").
//! Yields to the focus timer when a session is active.
//!
//! ## Tick policy
//!
//! The loop is **event-driven** with a state-dependent sleep budget:
//!
//! - countdown text visible (the digits decrement every second) → 1 s tick
//! - voice mode active (text changes only on phase transitions)  → 2 s tick
//! - focus mode active (focus_timer owns the title)              → 60 s tick
//! - truly idle (no countdown, no focus, no voice)               → 1 h tick
//!
//! In every state the loop additionally waits on a shared [`tokio::sync::Notify`]
//! that any state-changing site can poke via [`wake`]. Bus events
//! (`AlarmFired`, `TaskFocusChanged`, etc.), focus_timer start/end, and
//! voice phase transitions all wake immediately. The state-dependent sleep
//! is therefore a **safety upper bound**, not a polling interval.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, OnceLock};

use tauri::{AppHandle, Manager};
use tokio::sync::Notify;

use crate::app_core::AppCore;

/// Shared flag: when `true`, the focus timer owns the tray title.
pub static FOCUS_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Shared flag: when `true`, a voice capture session is active.
pub static VOICE_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Current voice conversation phase: 0=idle, 1=listening, 2=reflecting, 3=speaking.
pub static VOICE_PHASE: AtomicU8 = AtomicU8::new(0);

/// Shared wake handle. Initialized lazily by [`spawn`]; pokeable via [`wake`].
static WAKE: OnceLock<Arc<Notify>> = OnceLock::new();

/// Wake the countdown loop on next opportunity. No-op if the loop hasn't
/// been spawned yet (e.g. test environment). Safe to call from any thread.
pub fn wake() {
    if let Some(n) = WAKE.get() {
        n.notify_one();
    }
}

/// Cached "next item" — either a calendar event or a task deadline.
struct NextItem {
    title: String,
    time: jiff::Timestamp,
}

/// Spawn the background countdown loop. Call once during app setup.
///
/// `bus` is used to subscribe to domain events that invalidate the cached
/// "next item" so we avoid 30 s DB polling when nothing has changed.
pub fn spawn(
    app: &AppHandle,
    shutdown: tokio_util::sync::CancellationToken,
    bus: Arc<bus::DomainEventBus>,
) {
    // Start dirty so we load on the very first tick.
    let dirty = Arc::new(AtomicBool::new(true));
    let notify = WAKE.get_or_init(|| Arc::new(Notify::new())).clone();

    // Bus subscriber — marks dirty on any event that affects the next item
    // and pokes the loop so it re-evaluates immediately.
    let dirty_sub = Arc::clone(&dirty);
    let notify_sub = Arc::clone(&notify);
    let shutdown_sub = shutdown.clone();
    tauri::async_runtime::spawn(async move {
        let mut rx = bus.subscribe();
        loop {
            tokio::select! {
                _ = shutdown_sub.cancelled() => break,
                evt = rx.recv() => match evt {
                    Ok(e) if is_cache_invalidating(&e) => {
                        dirty_sub.store(true, Ordering::Relaxed);
                        notify_sub.notify_one();
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        // Lagged — mark dirty to be safe.
                        dirty_sub.store(true, Ordering::Relaxed);
                        notify_sub.notify_one();
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    });

    // Display loop — event-driven with state-dependent sleep upper bound.
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        countdown_loop(app, shutdown, dirty, notify).await;
    });
}

/// Returns `true` for domain events that should invalidate the cached next item.
fn is_cache_invalidating(evt: &bus::DomainEvent) -> bool {
    matches!(
        evt,
        bus::DomainEvent::Task(bus::TaskEvent::TaskFocusChanged { .. })
            | bus::DomainEvent::Alarm(bus::AlarmEvent::AlarmFired { .. })
            | bus::DomainEvent::Alarm(bus::AlarmEvent::AlarmSnoozed { .. })
            | bus::DomainEvent::Alarm(bus::AlarmEvent::AlarmCancelled { .. })
            | bus::DomainEvent::Productivity(bus::ProductivityEvent::FocusSessionStarted { .. })
            | bus::DomainEvent::Productivity(bus::ProductivityEvent::FocusSessionEnded { .. })
    )
}

/// Notify the countdown that the focus timer ended — clears the tray title
/// and wakes the loop so it re-evaluates the next item immediately.
pub fn notify_focus_ended(app: &AppHandle) {
    FOCUS_ACTIVE.store(false, Ordering::Relaxed);
    if let Some(tray) = app.tray_by_id("klynt-tray") {
        let _ = tray.set_title(Some(""));
    }
    wake();
}

/// Sleep budget when truly idle — long upper bound, in practice the bus
/// subscriber will wake us on every relevant change.
const IDLE_MAX_SLEEP_SECS: u64 = 3600;
/// Sleep when only focus is active (focus_timer owns the title; we just
/// need to detect the FOCUS_ACTIVE→false transition if the wake was missed).
const FOCUS_MAX_SLEEP_SECS: u64 = 60;
/// Sleep during voice mode — title changes only on phase transitions, but
/// VOICE_PHASE.store callers also call [`wake`].
const VOICE_TICK_SECS: u64 = 2;
/// Display tick when the countdown digits are visible.
const COUNTDOWN_TICK_SECS: u64 = 1;

async fn countdown_loop(
    app: AppHandle,
    shutdown: tokio_util::sync::CancellationToken,
    dirty: Arc<AtomicBool>,
    notify: Arc<Notify>,
) {
    let mut cached: Option<NextItem> = None;
    let mut last_tooltip: Option<String> = None;
    let mut set_tooltip = |app: &AppHandle, value: &str| {
        if last_tooltip.as_deref() != Some(value) {
            set_tray_tooltip(app, value);
            last_tooltip = Some(value.to_string());
        }
    };

    loop {
        // ── Render this tick's title and decide the next sleep budget. ──
        let sleep_secs;

        // DND session check — highest priority, overrides focus timer and voice.
        let dnd_session = query_dnd_session(&app).await;
        if let Some(ends_at) = dnd_session {
            let remaining_ms = ends_at.as_millisecond() - jiff::Timestamp::now().as_millisecond();
            if remaining_ms > 0 {
                let total_secs = remaining_ms / 1000;
                let hrs = total_secs / 3600;
                let mins = (total_secs % 3600) / 60;
                let title = if hrs > 0 {
                    format!("\u{1F319} {}h {:02}m", hrs, mins)
                } else {
                    format!("\u{1F319} {}m", mins.max(1))
                };
                set_tray_title(&app, &title);
                set_tooltip(&app, "DND active — click to manage");
                cached = None;
                dirty.store(true, Ordering::Relaxed);
                sleep_secs = COUNTDOWN_TICK_SECS;
            } else {
                // Session just expired — fall through to re-query normally.
                dirty.store(true, Ordering::Relaxed);
                sleep_secs = COUNTDOWN_TICK_SECS;
            }
        } else if FOCUS_ACTIVE.load(Ordering::Relaxed) {
            // Focus timer owns the title — drop our cached countdown so we
            // re-query when focus ends.
            cached = None;
            dirty.store(true, Ordering::Relaxed);
            sleep_secs = FOCUS_MAX_SLEEP_SECS;
        } else if VOICE_ACTIVE.load(Ordering::Relaxed) {
            let phase = VOICE_PHASE.load(Ordering::Relaxed);
            let title = match phase {
                1 => "Listening...",
                2 => "Reflecting...",
                3 => "Speaking...",
                _ => "Voice active",
            };
            set_tray_title(&app, title);
            set_tooltip(&app, "Voice active — click to pause");
            cached = None;
            sleep_secs = VOICE_TICK_SECS;
        } else {
            // Voice idle — hint tooltip.
            if VOICE_PHASE.load(Ordering::Relaxed) == 0 {
                if let Some(core) = app.try_state::<Arc<AppCore>>() {
                    if core.voice_service().is_ok() {
                        set_tooltip(&app, "Voice ready — ⌥⇧V to think out loud");
                    } else {
                        set_tooltip(&app, "Klynt");
                    }
                }
            }

            // Re-query DB on first run or when the bus subscriber marked dirty.
            if cached.is_none() || dirty.swap(false, Ordering::Relaxed) {
                cached = query_next_item(&app).await;
            }

            match &cached {
                Some(item) => {
                    let total_secs = (item.time.as_millisecond()
                        - jiff::Timestamp::now().as_millisecond())
                        / 1000;
                    if total_secs <= 0 {
                        // Item passed — clear and re-query next iteration.
                        set_tray_title(&app, "");
                        cached = None;
                        dirty.store(true, Ordering::Relaxed);
                        sleep_secs = COUNTDOWN_TICK_SECS;
                    } else {
                        let hrs = total_secs / 3600;
                        let mins = (total_secs % 3600) / 60;
                        let secs = total_secs % 60;
                        let truncated: String = item.title.chars().take(20).collect();
                        let time_str = if hrs > 0 {
                            format!("{hrs}:{mins:02}:{secs:02}")
                        } else {
                            format!("{mins:02}:{secs:02}")
                        };
                        set_tray_title(&app, &format!("« {time_str} · {truncated}"));
                        sleep_secs = COUNTDOWN_TICK_SECS;
                    }
                }
                None => {
                    set_tray_title(&app, "");
                    // Truly idle — sleep deeply, bus subscriber will wake us.
                    sleep_secs = IDLE_MAX_SLEEP_SECS;
                }
            }
        }

        // ── Wait: shutdown | wake | timeout (whichever first). ──
        tokio::select! {
            _ = shutdown.cancelled() => break,
            _ = notify.notified() => {}
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(sleep_secs)) => {}
        }
    }
}

/// Query both the next calendar event and the next task deadline, return whichever is sooner.
/// Only returns items due today — never shows countdowns for tomorrow or later.
async fn query_next_item(app: &AppHandle) -> Option<NextItem> {
    let core = app.try_state::<Arc<AppCore>>()?;
    // Use local time for "today" boundary so it matches the user's timezone
    let tomorrow = jiff::Zoned::now()
        .date()
        .checked_add(jiff::Span::new().days(1))
        .ok()?;
    let end_of_today = tomorrow
        .at(0, 0, 0, 0)
        .to_zoned(jiff::tz::TimeZone::system())
        .ok()?
        .timestamp();

    let (next_event_raw, next_task_raw) =
        tokio::join!(core.next_upcoming_event(), core.next_upcoming_task());

    let next_event = next_event_raw.and_then(|e| {
        let t = e.started_at.parse::<jiff::Timestamp>().ok()?;
        if t >= end_of_today {
            return None;
        }
        Some(NextItem {
            title: e.title,
            time: t,
        })
    });

    let next_task = next_task_raw.and_then(|t| {
        let due = t.due_date?;
        if *due >= end_of_today {
            return None;
        }
        Some(NextItem {
            title: t.title,
            time: *due,
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

/// Return the `ends_at` timestamp of the currently-active DND session, or `None`.
async fn query_dnd_session(app: &AppHandle) -> Option<jiff::Timestamp> {
    let core = app.try_state::<Arc<AppCore>>()?;
    let mgr = core.dnd_manager().ok()?;
    mgr.active(feature_focus::FocusMode::Dnd)
        .await
        .ok()?
        .map(|s| s.ends_at)
}

fn set_tray_title(app: &AppHandle, title: &str) {
    if let Some(tray) = app.tray_by_id("klynt-tray") {
        let _ = tray.set_title(Some(title));
    }
}

fn set_tray_tooltip(app: &AppHandle, tooltip: &str) {
    if let Some(tray) = app.tray_by_id("klynt-tray") {
        let _ = tray.set_tooltip(Some(tooltip));
    }
}
