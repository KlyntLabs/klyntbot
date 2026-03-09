//! Desktop focus timer — owns a 1-second tokio interval that updates the tray
//! icon title and emits tick events to the frontend. Delegates session
//! persistence to the existing `FocusManager` via `AppCore`.

use std::sync::Arc;

use desktop_shared::events::{FocusCompletedPayload, FocusTickPayload, FOCUS_COMPLETED, FOCUS_TICK};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::app_core::AppCore;
use crate::commands::window::WINDOW_TRAY;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerMode {
    Focus,
    Pomodoro,
}

impl TimerMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Focus => "focus",
            Self::Pomodoro => "pomodoro",
        }
    }
}

struct TimerState {
    mode: TimerMode,
    total_secs: u64,
    #[allow(dead_code)]
    break_mins: Option<u64>,
    handle: JoinHandle<()>,
}

pub struct FocusTimer {
    state: Mutex<Option<TimerState>>,
}

impl FocusTimer {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(None),
        }
    }

    /// Start a focus or pomodoro timer.
    ///
    /// The caller is responsible for starting the FocusManager session first.
    /// This method only manages the countdown, tray title, and completion events.
    pub async fn start(
        &self,
        app: AppHandle,
        mode: TimerMode,
        work_mins: u64,
        break_mins: Option<u64>,
    ) -> common::Result<()> {
        let mut guard = self.state.lock().await;
        if guard.is_some() {
            return Err(common::ToolError::ExecutionFailed(
                "Focus timer already running".into(),
            )
            .into());
        }

        let total_secs = work_mins * 60;
        let mode_str = mode.as_str().to_string();

        let handle = tokio::spawn(timer_loop(app, mode_str, total_secs, break_mins));

        *guard = Some(TimerState {
            mode,
            total_secs,
            break_mins,
            handle,
        });

        Ok(())
    }

    /// Stop the timer early. Returns `true` if a timer was running.
    pub async fn stop(&self, app: &AppHandle) -> bool {
        let mut guard = self.state.lock().await;
        if let Some(state) = guard.take() {
            state.handle.abort();
            clear_tray_title(app);
            true
        } else {
            false
        }
    }

    /// Get current timer status.
    pub async fn status(&self) -> Option<(TimerMode, u64)> {
        let guard = self.state.lock().await;
        guard.as_ref().map(|s| (s.mode, s.total_secs))
    }

    /// Called by the timer loop when it finishes naturally.
    /// Clears internal state without aborting the task.
    pub async fn mark_completed(&self) {
        let mut guard = self.state.lock().await;
        *guard = None;
    }
}

async fn timer_loop(app: AppHandle, mode: String, total_secs: u64, break_mins: Option<u64>) {
    let mut remaining = total_secs;
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));

    // Set initial tray title
    update_tray_title(&app, remaining);

    loop {
        interval.tick().await;

        if remaining == 0 {
            break;
        }
        remaining -= 1;

        // Update tray icon title with MM:SS
        update_tray_title(&app, remaining);

        // Emit tick event for frontend
        let _ = app.emit(
            FOCUS_TICK,
            FocusTickPayload {
                remaining_secs: remaining,
                total_secs,
                mode: mode.clone(),
            },
        );
    }

    // Timer complete — fire notification, sound, and auto-open tray
    on_timer_complete(&app, &mode, total_secs, break_mins).await;

    // Clear tray title
    clear_tray_title(&app);

    // Mark timer as done in FocusTimer state
    if let Some(timer) = app.try_state::<Arc<FocusTimer>>() {
        timer.mark_completed().await;
    }

    // Auto-end the focus session via AppCore
    if let Some(core) = app.try_state::<Arc<AppCore>>() {
        let _ = core.productivity_focus_end(None).await;
    }
}

fn update_tray_title(app: &AppHandle, remaining_secs: u64) {
    let mins = remaining_secs / 60;
    let secs = remaining_secs % 60;
    let title = format!("{mins:02}:{secs:02}");

    if let Some(tray) = app.tray_by_id("klynt-tray") {
        let _ = tray.set_title(Some(&title));
    }
}

fn clear_tray_title(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id("klynt-tray") {
        let _ = tray.set_title(None::<&str>);
    }
}

async fn on_timer_complete(
    app: &AppHandle,
    mode: &str,
    total_secs: u64,
    break_mins: Option<u64>,
) {
    let duration_mins = total_secs / 60;

    // Get quality score from the ended session
    let quality_score = if let Some(core) = app.try_state::<Arc<AppCore>>() {
        core.productivity_focus_status()
            .await
            .ok()
            .flatten()
            .and_then(|s| s.quality_score)
    } else {
        None
    };

    // 1. Fire OS notification
    let title = if mode == "pomodoro" {
        "Pomodoro Complete"
    } else {
        "Focus Session Complete"
    };

    let body = if mode == "pomodoro" {
        if let Some(brk) = break_mins {
            format!("{duration_mins} minute work session done. Time for a {brk} minute break!")
        } else {
            format!("{duration_mins} minute work session done. Take a break!")
        }
    } else if let Some(q) = quality_score {
        format!(
            "{duration_mins} minute session finished. Quality: {}%",
            (q * 100.0).round() as u32
        )
    } else {
        format!("{duration_mins} minute session finished.")
    };

    let _ = common::utils::notify::send_os_notification(title, &body).await;

    // 2. Play completion sound
    #[cfg(target_os = "macos")]
    {
        let _ = tokio::process::Command::new("afplay")
            .arg("/System/Library/Sounds/Glass.aiff")
            .spawn();
    }

    // 3. Emit completion event for frontend
    let _ = app.emit(
        FOCUS_COMPLETED,
        FocusCompletedPayload {
            mode: mode.to_string(),
            duration_mins,
            quality_score,
            break_mins,
        },
    );

    // 4. Auto-open tray window
    if let Some(window) = app.get_webview_window(WINDOW_TRAY) {
        if let Some(tray) = app.tray_by_id("klynt-tray") {
            if let Ok(Some(rect)) = tray.rect() {
                if let Ok(win_size) = window.outer_size() {
                    let scale = window.scale_factor().unwrap_or(1.0);
                    let tray_pos = rect.position.to_physical::<f64>(scale);
                    let tray_size = rect.size.to_physical::<f64>(scale);
                    let x = tray_pos.x + (tray_size.width / 2.0)
                        - (win_size.width as f64 / 2.0);
                    let y = tray_pos.y + tray_size.height;
                    let _ =
                        window.set_position(tauri::PhysicalPosition::new(x as i32, y as i32));
                }
            }
        }
        let _ = window.show();
        let _ = window.set_focus();
    }
}

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

        // Initially no timer
        assert!(timer.status().await.is_none());

        // Mark completed on empty does nothing
        timer.mark_completed().await;
        assert!(timer.status().await.is_none());
    }

    #[test]
    fn test_timer_mode_as_str() {
        assert_eq!(TimerMode::Focus.as_str(), "focus");
        assert_eq!(TimerMode::Pomodoro.as_str(), "pomodoro");
    }
}
