//! Desktop focus timer — owns a 1-second tokio interval that updates the tray
//! icon title and emits tick events to the frontend. Supports focus sessions,
//! break countdowns, pause/resume, and runtime extension via an mpsc command channel.

use std::sync::Arc;

use desktop_shared::commands::FocusSessionResponse;
use desktop_shared::events::{
    FocusCompletedPayload, FocusTickPayload, FOCUS_COMPLETED, FOCUS_TICK,
};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;

use crate::app_core::AppCore;
use crate::commands::window::WINDOW_TRAY;
use crate::tray_countdown;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerMode {
    Focus,
    Pomodoro,
    Break,
}

impl TimerMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Focus => "focus",
            Self::Pomodoro => "pomodoro",
            Self::Break => "break",
        }
    }
}

// ── Commands sent to the running timer loop ─────────────────────────

enum TimerCommand {
    Extend(u64),
    Pause,
    Resume,
}

struct TimerState {
    mode: TimerMode,
    total_secs: u64,
    #[allow(dead_code)]
    break_mins: Option<u64>,
    #[allow(dead_code)]
    action_title: Option<String>,
    sound_enabled: bool,
    notification_enabled: bool,
    handle: JoinHandle<()>,
    cmd_tx: mpsc::Sender<TimerCommand>,
}

// ── Public API ──────────────────────────────────────────────────────

pub struct FocusTimer {
    state: Mutex<Option<TimerState>>,
}

impl FocusTimer {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(None),
        }
    }

    /// Start a focus timer. Caller must start the FocusManager session first.
    #[allow(clippy::too_many_arguments)]
    pub async fn start(
        &self,
        app: AppHandle,
        mode: TimerMode,
        work_mins: u64,
        break_mins: Option<u64>,
        action_title: Option<String>,
        sound_enabled: bool,
        notification_enabled: bool,
    ) -> common::Result<()> {
        let mut guard = self.state.lock().await;
        if guard.is_some() {
            return Err(common::ToolError::ExecutionFailed("Timer already running".into()).into());
        }

        tray_countdown::FOCUS_ACTIVE.store(true, std::sync::atomic::Ordering::Relaxed);

        let total_secs = work_mins * 60;
        let (cmd_tx, cmd_rx) = mpsc::channel(8);
        let handle = tokio::spawn(timer_loop(
            app,
            mode.as_str().to_string(),
            total_secs,
            break_mins,
            action_title.clone(),
            cmd_rx,
        ));

        *guard = Some(TimerState {
            mode,
            total_secs,
            break_mins,
            action_title,
            sound_enabled,
            notification_enabled,
            handle,
            cmd_tx,
        });

        Ok(())
    }

    /// Start a break countdown. No focus session is created in AppCore.
    pub async fn start_break(&self, app: AppHandle, break_mins: u64) -> common::Result<()> {
        let mut guard = self.state.lock().await;
        if guard.is_some() {
            return Err(common::ToolError::ExecutionFailed("Timer already running".into()).into());
        }

        tray_countdown::FOCUS_ACTIVE.store(true, std::sync::atomic::Ordering::Relaxed);

        let total_secs = break_mins * 60;
        let (cmd_tx, cmd_rx) = mpsc::channel(8);
        let handle = tokio::spawn(timer_loop(
            app,
            TimerMode::Break.as_str().to_string(),
            total_secs,
            None,
            None,
            cmd_rx,
        ));

        *guard = Some(TimerState {
            mode: TimerMode::Break,
            total_secs,
            break_mins: None,
            action_title: None,
            sound_enabled: true,
            notification_enabled: true,
            handle,
            cmd_tx,
        });

        Ok(())
    }

    /// Extend the running timer by `extra_secs`.
    pub async fn extend(&self, extra_secs: u64) -> bool {
        self.send_command(TimerCommand::Extend(extra_secs)).await
    }

    /// Pause the running timer.
    pub async fn pause(&self) -> bool {
        self.send_command(TimerCommand::Pause).await
    }

    /// Resume the paused timer.
    pub async fn resume(&self) -> bool {
        self.send_command(TimerCommand::Resume).await
    }

    async fn send_command(&self, cmd: TimerCommand) -> bool {
        let guard = self.state.lock().await;
        match guard.as_ref() {
            Some(state) => state.cmd_tx.try_send(cmd).is_ok(),
            None => false,
        }
    }

    /// Stop the timer early.
    pub async fn stop(&self, app: &AppHandle) -> bool {
        let mut guard = self.state.lock().await;
        if let Some(state) = guard.take() {
            state.handle.abort();
            // Wait for the task to fully stop — avoids a race where the loop's
            // `update_tray_title` runs after our `clear_tray_title`.
            let _ = state.handle.await;
            clear_tray_title(app);
            tray_countdown::notify_focus_ended(app);
            true
        } else {
            false
        }
    }

    /// Get sound/notification preferences for the current session.
    pub async fn preferences(&self) -> (bool, bool) {
        let guard = self.state.lock().await;
        guard
            .as_ref()
            .map(|s| (s.sound_enabled, s.notification_enabled))
            .unwrap_or((true, true))
    }

    /// Get current timer status.
    pub async fn status(&self) -> Option<(TimerMode, u64)> {
        let guard = self.state.lock().await;
        guard.as_ref().map(|s| (s.mode, s.total_secs))
    }

    /// Called by the timer loop when it finishes naturally.
    pub async fn mark_completed(&self) {
        let mut guard = self.state.lock().await;
        *guard = None;
    }
}

// ── Timer loop ──────────────────────────────────────────────────────

const WARNING_SECS: u64 = 30;

async fn timer_loop(
    app: AppHandle,
    mode: String,
    mut total_secs: u64,
    break_mins: Option<u64>,
    action_title: Option<String>,
    mut cmd_rx: mpsc::Receiver<TimerCommand>,
) {
    // Pre-truncate once so the hot loop doesn't re-truncate every second
    let truncated_title: Option<String> = action_title.as_deref().and_then(|t| {
        if t.is_empty() {
            None
        } else {
            Some(t.chars().take(20).collect())
        }
    });
    let mut remaining = total_secs;
    let mut paused = false;
    let mut warning_shown = false;
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));

    update_tray_title(&app, remaining, false, truncated_title.as_deref());

    loop {
        // Drain any pending commands (non-blocking)
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                TimerCommand::Extend(secs) => {
                    remaining += secs;
                    total_secs += secs;
                    // Reset warning if we extended past the threshold
                    if remaining > WARNING_SECS {
                        warning_shown = false;
                    }
                }
                TimerCommand::Pause => paused = true,
                TimerCommand::Resume => paused = false,
            }
        }

        interval.tick().await;

        if paused {
            update_tray_title(&app, remaining, true, truncated_title.as_deref());
            let _ = app.emit(
                FOCUS_TICK,
                FocusTickPayload {
                    remaining_secs: remaining,
                    total_secs,
                    mode: mode.clone(),
                    paused: true,
                    action_title: action_title.clone(),
                },
            );
            continue;
        }

        if remaining == 0 {
            break;
        }
        remaining -= 1;

        // 30-second warning: pop open the tray window so user sees extend options
        if remaining == WARNING_SECS && !warning_shown {
            warning_shown = true;
            open_tray_window(&app);
        }

        update_tray_title(&app, remaining, false, truncated_title.as_deref());

        let _ = app.emit(
            FOCUS_TICK,
            FocusTickPayload {
                remaining_secs: remaining,
                total_secs,
                mode: mode.clone(),
                paused: false,
                action_title: action_title.clone(),
            },
        );
    }

    // Timer complete
    let is_break = mode == TimerMode::Break.as_str();

    if is_break {
        on_break_complete(&app).await;
    } else {
        // End the AppCore session FIRST (computes quality, emits DomainEvent::FocusSessionEnded)
        let ended_session = if let Some(core) = app.try_state::<Arc<AppCore>>() {
            core.productivity_focus_end(None).await.ok().flatten()
        } else {
            None
        };

        on_focus_complete(&app, &mode, total_secs, break_mins, ended_session.as_ref()).await;
    }

    clear_tray_title(&app);
    tray_countdown::notify_focus_ended(&app);

    if let Some(timer) = app.try_state::<Arc<FocusTimer>>() {
        timer.mark_completed().await;
    }
}

// ── Tray title helpers ──────────────────────────────────────────────

fn update_tray_title(
    app: &AppHandle,
    remaining_secs: u64,
    paused: bool,
    action_title: Option<&str>,
) {
    let mins = remaining_secs / 60;
    let secs = remaining_secs % 60;
    let time = if paused {
        format!("⏸ {mins:02}:{secs:02}")
    } else {
        format!("{mins:02}:{secs:02}")
    };
    let title = match action_title {
        Some(t) => format!("{time} · {t}"),
        None => time,
    };

    if let Some(tray) = app.tray_by_id("klynt-tray") {
        let _ = tray.set_title(Some(&title));
    }
}

fn clear_tray_title(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id("klynt-tray") {
        let _ = tray.set_title(Some(""));
    }
}

/// Read sound/notification preferences from the timer state, defaulting to
/// `(true, true)` if no timer is registered.
async fn read_preferences(app: &AppHandle) -> (bool, bool) {
    if let Some(timer) = app.try_state::<Arc<FocusTimer>>() {
        timer.preferences().await
    } else {
        (true, true)
    }
}

// ── Completion handlers ─────────────────────────────────────────────

pub fn open_tray_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW_TRAY) {
        let positioned = if let Some(tray) = app.tray_by_id("klynt-tray") {
            if let Ok(Some(rect)) = tray.rect() {
                if let Ok(win_size) = window.outer_size() {
                    let scale = window.scale_factor().unwrap_or(1.0);
                    let tray_pos = rect.position.to_physical::<f64>(scale);
                    let tray_size = rect.size.to_physical::<f64>(scale);
                    let x = tray_pos.x + (tray_size.width / 2.0) - (win_size.width as f64 / 2.0);
                    let y = tray_pos.y + tray_size.height;
                    let _ = window.set_position(tauri::PhysicalPosition::new(x as i32, y as i32));
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        // Fallback: if tray rect unavailable (sleep/wake, first launch), position
        // at top-center of the primary monitor near the menu bar.
        if !positioned {
            if let Ok(Some(monitor)) = window.primary_monitor() {
                let screen = monitor.size();
                let scale = monitor.scale_factor();
                let win_width = window
                    .outer_size()
                    .map(|s| s.width as f64)
                    .unwrap_or(320.0 * scale);
                let x = (screen.width as f64 / 2.0) - (win_width / 2.0);
                let y = 28.0 * scale; // Below the macOS menu bar (~28 logical pts)
                let _ = window.set_position(tauri::PhysicalPosition::new(x as i32, y as i32));
            }
        }

        let _ = window.show();
        let _ = window.set_focus();
    }
}

async fn on_focus_complete(
    app: &AppHandle,
    mode: &str,
    total_secs: u64,
    break_mins: Option<u64>,
    ended_session: Option<&FocusSessionResponse>,
) {
    let duration_mins = total_secs / 60;
    let quality_score = ended_session.and_then(|s| s.quality_score);

    let (sound_enabled, notification_enabled) = read_preferences(app).await;

    // Notification
    if notification_enabled {
        let body = match (break_mins, quality_score) {
            (Some(brk), Some(q)) => format!(
                "{duration_mins}m done (quality {}%). Take a {brk}m break!",
                (q * 100.0).round() as u32
            ),
            (Some(brk), None) => {
                format!("{duration_mins}m session done. Time for a {brk}m break!")
            }
            (None, Some(q)) => format!(
                "{duration_mins}m session finished. Quality: {}%",
                (q * 100.0).round() as u32
            ),
            (None, None) => format!("{duration_mins}m session finished."),
        };
        let _ = crate::notify::TauriNotificationSender::new(app.clone())
            .send_sync("Focus Session Complete", &body);
    }

    // Sound
    #[cfg(target_os = "macos")]
    if sound_enabled {
        let _ = tokio::process::Command::new("afplay")
            .arg("/System/Library/Sounds/Glass.aiff")
            .spawn();
    }

    // Frontend event
    let _ = app.emit(
        FOCUS_COMPLETED,
        FocusCompletedPayload {
            mode: mode.to_string(),
            duration_mins,
            quality_score,
            break_mins,
        },
    );

    open_tray_window(app);
}

async fn on_break_complete(app: &AppHandle) {
    // End the break session in SQLite
    if let Some(core) = app.try_state::<Arc<AppCore>>() {
        let _ = core.productivity_break_end().await;
    }

    let (sound_enabled, notification_enabled) = read_preferences(app).await;

    if notification_enabled {
        let _ = crate::notify::TauriNotificationSender::new(app.clone())
            .send_sync("Break Over", "Ready for the next focus session!");
    }

    // Softer sound for break end
    #[cfg(target_os = "macos")]
    if sound_enabled {
        let _ = tokio::process::Command::new("afplay")
            .arg("/System/Library/Sounds/Blow.aiff")
            .spawn();
    }

    let _ = app.emit(
        FOCUS_COMPLETED,
        FocusCompletedPayload {
            mode: TimerMode::Break.as_str().to_string(),
            duration_mins: 0,
            quality_score: None,
            break_mins: None,
        },
    );

    open_tray_window(app);
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_tray_title() {
        assert_eq!(format!("{:02}:{:02}", 25u64, 0u64), "25:00");
        assert_eq!(format!("{:02}:{:02}", 0u64, 5u64), "00:05");
        assert_eq!(format!("{:02}:{:02}", 1u64, 30u64), "01:30");
    }

    #[tokio::test]
    async fn test_timer_state_machine() {
        let timer = FocusTimer::new();
        assert!(timer.status().await.is_none());
        timer.mark_completed().await;
        assert!(timer.status().await.is_none());
    }

    #[test]
    fn test_timer_mode_as_str() {
        assert_eq!(TimerMode::Focus.as_str(), "focus");
        assert_eq!(TimerMode::Pomodoro.as_str(), "pomodoro");
        assert_eq!(TimerMode::Break.as_str(), "break");
    }
}
