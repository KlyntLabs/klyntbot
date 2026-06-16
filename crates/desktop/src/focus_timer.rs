//! Desktop focus timer — backend-owned phase state machine with 1-second sync
//! ticks, macOS DND integration, and automatic cycle progression.
//!
//! Phases: Working → BreakPending (5s) → Break → Working (auto-continues).
//! The backend owns cycle position and break-type decisions.

use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_shared::events::{
    FocusDndUnavailablePayload, FocusSyncPayload, FocusWarningPayload, FOCUS_PHASE_CHANGED,
};
use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use tracing::warn;

use crate::app_core::AppCore;
use crate::commands::window::WINDOW_TRAY;
use crate::tray_countdown;

// ── Configuration ────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct FocusSessionConfig {
    pub work_secs: u64,
    pub short_break_secs: u64,
    pub long_break_secs: u64,
    pub long_break_after: u32,
}

// ── Phase state ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum Phase {
    Working {
        remaining: u64,
        total: u64,
    },
    BreakPending {
        remaining: u64,
    },
    Break {
        remaining: u64,
        total: u64,
    },
    Suspended {
        previous: SuspendedFrom,
        remaining: u64,
        total: u64,
    },
}

/// What phase was active before suspension (avoids stringly-typed state).
#[derive(Debug, Clone)]
enum SuspendedFrom {
    Working,
    BreakPending,
    Break,
}

impl Phase {
    fn as_str(&self) -> &'static str {
        match self {
            Phase::Working { .. } => "working",
            Phase::BreakPending { .. } => "break_pending",
            Phase::Break { .. } => "break",
            Phase::Suspended { .. } => "suspended",
        }
    }

    fn remaining(&self) -> u64 {
        match self {
            Phase::Working { remaining, .. }
            | Phase::BreakPending { remaining }
            | Phase::Break { remaining, .. }
            | Phase::Suspended { remaining, .. } => *remaining,
        }
    }

    fn total(&self) -> u64 {
        match self {
            Phase::Working { total, .. }
            | Phase::Break { total, .. }
            | Phase::Suspended { total, .. } => *total,
            Phase::BreakPending { remaining } => *remaining,
        }
    }

    fn decrement(&mut self) {
        match self {
            Phase::Working { remaining, .. }
            | Phase::BreakPending { remaining }
            | Phase::Break { remaining, .. } => {
                *remaining = remaining.saturating_sub(1);
            }
            Phase::Suspended { .. } => {} // frozen
        }
    }
}

// ── Commands sent to the running timer loop ─────────────────────────

pub enum SessionCommand {
    Pause,
    Resume,
    Extend(u64),
    StartBreak,
    ExtendWork(u64),
    SkipBreak,
    TakeBreak,
    Suspend,
    ResumeSuspended,
}

// ── Session state (shared between timer loop and public API) ────────

struct SessionState {
    config: FocusSessionConfig,
    #[allow(dead_code)]
    cycle_position: u32,
    dnd_enabled: bool,
    dnd_was_active_before: bool,
    sound_enabled: bool,
    notification_enabled: bool,
    #[allow(dead_code)]
    action_title: Option<String>,
    #[allow(dead_code)]
    action_id: Option<String>,
    handle: JoinHandle<()>,
    cmd_tx: mpsc::Sender<SessionCommand>,
}

// ── Public API ──────────────────────────────────────────────────────

pub struct FocusTimer {
    state: Mutex<Option<SessionState>>,
}

impl Default for FocusTimer {
    fn default() -> Self {
        Self {
            state: Mutex::new(None),
        }
    }
}

impl FocusTimer {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn start(
        &self,
        app: AppHandle,
        config: FocusSessionConfig,
        action_id: Option<String>,
        action_title: Option<String>,
        dnd_enabled: bool,
        sound_enabled: bool,
        notification_enabled: bool,
    ) -> common::Result<()> {
        let mut guard = self.state.lock().await;

        // Clean up any stale session (e.g. from a previous run or app restart)
        if let Some(old) = guard.take() {
            old.handle.abort();
            let _ = old.handle.await;
            clear_tray_title(&app);
            tray_countdown::notify_focus_ended(&app);
            restore_dnd_state(old.dnd_enabled, old.dnd_was_active_before);
        }

        tray_countdown::FOCUS_ACTIVE.store(true, std::sync::atomic::Ordering::Relaxed);
        tray_countdown::wake();

        // DND: capture and enable
        let dnd_was_active_before = if dnd_enabled {
            let was = platform_macos::dnd::is_dnd_active();
            if !was {
                if let Err(e) = platform_macos::dnd::toggle_dnd() {
                    warn!("Failed to enable DND: {e}");
                    use tauri_specta::Event;
                    let payload = FocusDndUnavailablePayload {
                        message: format!(
                            "Could not enable Do Not Disturb: {e}. \
                             Create a Shortcut named 'Toggle Do Not Disturb' to enable this."
                        ),
                    };
                    let _ = payload.emit(&app);
                }
            }
            was
        } else {
            false
        };

        let (cmd_tx, cmd_rx) = mpsc::channel(16);
        let handle = tokio::spawn(session_loop(
            app,
            config.clone(),
            action_title.clone(),
            dnd_enabled,
            cmd_rx,
        ));

        *guard = Some(SessionState {
            config,
            cycle_position: 0,
            dnd_enabled,
            dnd_was_active_before,
            sound_enabled,
            notification_enabled,
            action_title,
            action_id,
            handle,
            cmd_tx,
        });

        Ok(())
    }

    pub async fn suspend(&self) {
        if let Some(state) = self.state.lock().await.as_ref() {
            let _ = state.cmd_tx.send(SessionCommand::Suspend).await;
        }
    }

    pub async fn resume_suspended(&self) {
        if let Some(state) = self.state.lock().await.as_ref() {
            let _ = state.cmd_tx.send(SessionCommand::ResumeSuspended).await;
        }
    }

    pub async fn send_command(&self, cmd: SessionCommand) -> bool {
        let guard = self.state.lock().await;
        match guard.as_ref() {
            Some(state) => state.cmd_tx.try_send(cmd).is_ok(),
            None => false,
        }
    }

    pub async fn stop(&self, app: &AppHandle) -> bool {
        let mut guard = self.state.lock().await;
        if let Some(state) = guard.take() {
            let dnd_enabled = state.dnd_enabled;
            let dnd_was_active_before = state.dnd_was_active_before;
            state.handle.abort();
            let _ = state.handle.await;
            clear_tray_title(app);
            tray_countdown::notify_focus_ended(app);
            restore_dnd_state(dnd_enabled, dnd_was_active_before);
            true
        } else {
            false
        }
    }

    pub async fn status(&self) -> Option<FocusSessionConfig> {
        let guard = self.state.lock().await;
        guard.as_ref().map(|s| s.config.clone())
    }

    pub async fn preferences(&self) -> (bool, bool) {
        let guard = self.state.lock().await;
        guard
            .as_ref()
            .map(|s| (s.sound_enabled, s.notification_enabled))
            .unwrap_or((true, true))
    }
}

fn restore_dnd_state(dnd_enabled: bool, dnd_was_active_before: bool) {
    if dnd_enabled && !dnd_was_active_before {
        if let Err(e) = platform_macos::dnd::toggle_dnd() {
            warn!("Failed to restore DND: {e}");
        }
    }
}

// ── Session loop ─────────────────────────────────────────────────────

const WARNING_SECS: u64 = 30;
const BREAK_PENDING_SECS: u64 = 5;
const SYNC_INTERVAL: u64 = 1;

async fn session_loop(
    app: AppHandle,
    config: FocusSessionConfig,
    action_title: Option<String>,
    dnd_enabled: bool,
    mut cmd_rx: mpsc::Receiver<SessionCommand>,
) {
    let truncated_title: Option<String> = action_title.as_deref().and_then(|t| {
        if t.is_empty() {
            None
        } else {
            Some(t.chars().take(20).collect())
        }
    });

    let mut phase = Phase::Working {
        remaining: config.work_secs,
        total: config.work_secs,
    };
    let mut cycle_position: u32 = 0;
    let mut paused = false;
    let mut warning_shown = false;
    let mut sync_counter: u64 = 0;
    let tick_duration = Duration::from_secs(1);
    let mut tick_count: u64 = 0;
    let mut segment_start = Instant::now();

    // Emit initial phase_changed
    emit_phase_changed(
        &app,
        &phase,
        cycle_position,
        &config,
        paused,
        truncated_title.as_deref(),
        dnd_enabled,
    );
    update_tray_title(&app, phase.remaining(), paused, truncated_title.as_deref());

    loop {
        // Drain pending commands
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                SessionCommand::Pause => {
                    paused = true;
                    update_tray_title(&app, phase.remaining(), true, truncated_title.as_deref());
                    emit_phase_changed(
                        &app,
                        &phase,
                        cycle_position,
                        &config,
                        true,
                        truncated_title.as_deref(),
                        dnd_enabled,
                    );
                }
                SessionCommand::Resume => {
                    paused = false;
                    emit_phase_changed(
                        &app,
                        &phase,
                        cycle_position,
                        &config,
                        false,
                        truncated_title.as_deref(),
                        dnd_enabled,
                    );
                }
                SessionCommand::Extend(secs) => {
                    if let Phase::Working { remaining, total } | Phase::Break { remaining, total } =
                        &mut phase
                    {
                        *remaining += secs;
                        *total += secs;
                        if *remaining > WARNING_SECS {
                            warning_shown = false;
                        }
                    }
                    reset_segment(&mut segment_start, &mut tick_count);
                    emit_phase_changed(
                        &app,
                        &phase,
                        cycle_position,
                        &config,
                        paused,
                        truncated_title.as_deref(),
                        dnd_enabled,
                    );
                }
                SessionCommand::StartBreak => {
                    if matches!(phase, Phase::BreakPending { .. }) {
                        let break_secs = compute_break_secs(&config, cycle_position);
                        phase = Phase::Break {
                            remaining: break_secs,
                            total: break_secs,
                        };
                        warning_shown = false;
                        sync_counter = 0;
                        reset_segment(&mut segment_start, &mut tick_count);
                        // Start break session in AppCore
                        start_break_session(&app, break_secs).await;
                        emit_phase_changed(
                            &app,
                            &phase,
                            cycle_position,
                            &config,
                            paused,
                            truncated_title.as_deref(),
                            dnd_enabled,
                        );
                    }
                }
                SessionCommand::ExtendWork(secs) => {
                    if matches!(phase, Phase::BreakPending { .. }) {
                        phase = Phase::Working {
                            remaining: secs,
                            total: secs,
                        };
                        warning_shown = false;
                        sync_counter = 0;
                        reset_segment(&mut segment_start, &mut tick_count);
                        emit_phase_changed(
                            &app,
                            &phase,
                            cycle_position,
                            &config,
                            paused,
                            truncated_title.as_deref(),
                            dnd_enabled,
                        );
                    }
                }
                SessionCommand::SkipBreak => {
                    if matches!(phase, Phase::Break { .. } | Phase::BreakPending { .. }) {
                        // End break session if in Break
                        if matches!(phase, Phase::Break { .. }) {
                            end_break_session(&app).await;
                        }
                        // Start next focus cycle
                        start_focus_session(&app, &config).await;
                        phase = Phase::Working {
                            remaining: config.work_secs,
                            total: config.work_secs,
                        };
                        warning_shown = false;
                        sync_counter = 0;
                        reset_segment(&mut segment_start, &mut tick_count);
                        emit_phase_changed(
                            &app,
                            &phase,
                            cycle_position,
                            &config,
                            paused,
                            truncated_title.as_deref(),
                            dnd_enabled,
                        );
                    }
                }
                SessionCommand::TakeBreak => {
                    if matches!(phase, Phase::Working { .. }) {
                        // End the current focus session
                        end_focus_session(&app).await;
                        // Enter break_pending immediately
                        phase = Phase::BreakPending {
                            remaining: BREAK_PENDING_SECS,
                        };
                        warning_shown = false;
                        sync_counter = 0;
                        reset_segment(&mut segment_start, &mut tick_count);
                        open_tray_window(&app);
                        emit_phase_changed(
                            &app,
                            &phase,
                            cycle_position,
                            &config,
                            paused,
                            truncated_title.as_deref(),
                            dnd_enabled,
                        );
                    }
                }
                SessionCommand::Suspend => {
                    if !matches!(phase, Phase::Suspended { .. }) {
                        let previous = match &phase {
                            Phase::Working { .. } => SuspendedFrom::Working,
                            Phase::BreakPending { .. } => SuspendedFrom::BreakPending,
                            Phase::Break { .. } => SuspendedFrom::Break,
                            Phase::Suspended { .. } => unreachable!(),
                        };
                        let remaining = phase.remaining();
                        let total = phase.total();
                        phase = Phase::Suspended {
                            previous,
                            remaining,
                            total,
                        };
                        reset_segment(&mut segment_start, &mut tick_count);
                        update_tray_title(
                            &app,
                            phase.remaining(),
                            paused,
                            truncated_title.as_deref(),
                        );
                        emit_phase_changed(
                            &app,
                            &phase,
                            cycle_position,
                            &config,
                            paused,
                            truncated_title.as_deref(),
                            dnd_enabled,
                        );
                    }
                }
                SessionCommand::ResumeSuspended => {
                    if let Phase::Suspended {
                        previous,
                        remaining,
                        total,
                    } = &phase
                    {
                        let rem = *remaining;
                        let tot = *total;
                        phase = match previous {
                            SuspendedFrom::BreakPending => Phase::BreakPending { remaining: rem },
                            SuspendedFrom::Break => Phase::Break {
                                remaining: rem,
                                total: tot,
                            },
                            SuspendedFrom::Working => Phase::Working {
                                remaining: rem,
                                total: tot,
                            },
                        };
                        paused = false;
                        reset_segment(&mut segment_start, &mut tick_count);
                        emit_phase_changed(
                            &app,
                            &phase,
                            cycle_position,
                            &config,
                            false,
                            truncated_title.as_deref(),
                            dnd_enabled,
                        );
                    }
                }
            }
        }

        let ticks = u32::try_from(tick_count.saturating_add(1)).unwrap_or(u32::MAX);
        let next_tick = segment_start + tick_duration * ticks;
        let sleep_for = next_tick.saturating_duration_since(Instant::now());
        tokio::time::sleep(sleep_for).await;
        tick_count += 1;

        if paused {
            // Don't emit sync while paused — frontend freezes display
            update_tray_title(&app, phase.remaining(), true, truncated_title.as_deref());
            continue;
        }

        if phase.remaining() == 0 {
            // Phase transition
            match &phase {
                Phase::Working { total, .. } => {
                    // End the AppCore focus session
                    end_focus_session(&app).await;

                    // Play sound/notification for work completion
                    on_work_complete(&app, *total / 60).await;

                    // Enter break_pending
                    phase = Phase::BreakPending {
                        remaining: BREAK_PENDING_SECS,
                    };
                    warning_shown = false;
                    sync_counter = 0;
                    reset_segment(&mut segment_start, &mut tick_count);
                    open_tray_window(&app);
                    emit_phase_changed(
                        &app,
                        &phase,
                        cycle_position,
                        &config,
                        paused,
                        truncated_title.as_deref(),
                        dnd_enabled,
                    );
                }
                Phase::BreakPending { .. } => {
                    // Auto-transition to break
                    let break_secs = compute_break_secs(&config, cycle_position);
                    start_break_session(&app, break_secs).await;
                    phase = Phase::Break {
                        remaining: break_secs,
                        total: break_secs,
                    };
                    warning_shown = false;
                    sync_counter = 0;
                    reset_segment(&mut segment_start, &mut tick_count);
                    emit_phase_changed(
                        &app,
                        &phase,
                        cycle_position,
                        &config,
                        paused,
                        truncated_title.as_deref(),
                        dnd_enabled,
                    );
                }
                Phase::Break { .. } => {
                    // Break complete — end break session in AppCore
                    end_break_session(&app).await;
                    on_break_complete(&app).await;

                    // Increment cycle, reset after long break
                    cycle_position += 1;
                    let was_long = is_long_break(cycle_position.saturating_sub(1), &config);
                    if was_long {
                        cycle_position = 0;
                    }

                    // Auto-start next focus session
                    start_focus_session(&app, &config).await;
                    phase = Phase::Working {
                        remaining: config.work_secs,
                        total: config.work_secs,
                    };
                    warning_shown = false;
                    sync_counter = 0;
                    reset_segment(&mut segment_start, &mut tick_count);
                    emit_phase_changed(
                        &app,
                        &phase,
                        cycle_position,
                        &config,
                        paused,
                        truncated_title.as_deref(),
                        dnd_enabled,
                    );
                }
                Phase::Suspended { .. } => {
                    // Suspended phase never decrements — remaining() == 0 here
                    // means it was suspended at 0s; just wait for ResumeSuspended.
                }
            }
            continue;
        }

        phase.decrement();

        // Warning at 30 seconds remaining (Working and Break phases only)
        if !warning_shown
            && phase.remaining() == WARNING_SECS
            && !matches!(phase, Phase::BreakPending { .. } | Phase::Suspended { .. })
        {
            warning_shown = true;
            open_tray_window(&app);
            use tauri_specta::Event;
            let payload = FocusWarningPayload {
                phase: phase.as_str().to_string(),
                remaining_secs: phase.remaining(),
            };
            let _ = payload.emit(&app);
        }

        update_tray_title(&app, phase.remaining(), false, truncated_title.as_deref());

        // Sync event: every 1s for BreakPending (it's only 5s), every 5s otherwise
        let should_sync = match &phase {
            Phase::BreakPending { .. } => true,
            _ => {
                sync_counter += 1;
                if sync_counter >= SYNC_INTERVAL {
                    sync_counter = 0;
                    true
                } else {
                    false
                }
            }
        };

        if should_sync {
            use tauri_specta::Event;
            let payload = build_sync_payload(
                &phase,
                cycle_position,
                &config,
                paused,
                truncated_title.as_deref(),
                dnd_enabled,
            );
            let _ = payload.emit(&app);
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

fn reset_segment(segment_start: &mut Instant, tick_count: &mut u64) {
    *segment_start = Instant::now();
    *tick_count = 0;
}

fn compute_break_secs(config: &FocusSessionConfig, cycle_position: u32) -> u64 {
    if is_long_break(cycle_position, config) {
        config.long_break_secs
    } else {
        config.short_break_secs
    }
}

#[allow(clippy::manual_is_multiple_of)] // is_multiple_of requires Rust 1.83+, MSRV is 1.75
fn is_long_break(cycle_position: u32, config: &FocusSessionConfig) -> bool {
    config.long_break_after > 0 && (cycle_position + 1) % config.long_break_after == 0
}

fn build_sync_payload(
    phase: &Phase,
    cycle_position: u32,
    config: &FocusSessionConfig,
    paused: bool,
    action_title: Option<&str>,
    dnd_active: bool,
) -> FocusSyncPayload {
    FocusSyncPayload {
        phase: phase.as_str().to_string(),
        remaining_secs: phase.remaining(),
        total_secs: phase.total(),
        cycle_position,
        long_break_after: config.long_break_after,
        paused,
        action_title: action_title.map(|s| s.to_string()),
        dnd_active,
    }
}

fn emit_phase_changed(
    app: &AppHandle,
    phase: &Phase,
    cycle_position: u32,
    config: &FocusSessionConfig,
    paused: bool,
    action_title: Option<&str>,
    dnd_active: bool,
) {
    let _ = app.emit(
        FOCUS_PHASE_CHANGED,
        build_sync_payload(
            phase,
            cycle_position,
            config,
            paused,
            action_title,
            dnd_active,
        ),
    );
}

// ── AppCore delegates ────────────────────────────────────────────────

async fn start_focus_session(app: &AppHandle, config: &FocusSessionConfig) {
    if let Some(core) = app.try_state::<Arc<AppCore>>() {
        let _ = core
            .productivity_focus_start(None, None, Some(config.work_secs as i64 / 60))
            .await;
    }
}

async fn end_focus_session(app: &AppHandle) {
    if let Some(core) = app.try_state::<Arc<AppCore>>() {
        let _ = core.productivity_focus_end(None).await;
    }
}

async fn start_break_session(app: &AppHandle, break_secs: u64) {
    if let Some(core) = app.try_state::<Arc<AppCore>>() {
        let _ = core.productivity_break_start(break_secs as i64 / 60).await;
    }
}

async fn end_break_session(app: &AppHandle) {
    if let Some(core) = app.try_state::<Arc<AppCore>>() {
        let _ = core.productivity_break_end().await;
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
        format!("\u{23F8} {mins:02}:{secs:02}")
    } else {
        format!("{mins:02}:{secs:02}")
    };
    let title = match action_title {
        Some(t) => format!("{time} \u{00B7} {t}"),
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

/// Read sound/notification preferences from the timer state.
async fn read_preferences(app: &AppHandle) -> (bool, bool) {
    if let Some(timer) = app.try_state::<Arc<FocusTimer>>() {
        timer.preferences().await
    } else {
        (true, true)
    }
}

// ── Completion handlers ─────────────────────────────────────────────

pub fn open_tray_window(app: &AppHandle) {
    if let Some(window) = crate::lazy_window::get_or_create_window(app, WINDOW_TRAY) {
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

        if !positioned {
            if let Ok(Some(monitor)) = window.primary_monitor() {
                let screen = monitor.size();
                let scale = monitor.scale_factor();
                let win_width = window
                    .outer_size()
                    .map(|s| s.width as f64)
                    .unwrap_or(320.0 * scale);
                let x = (screen.width as f64 / 2.0) - (win_width / 2.0);
                let y = 28.0 * scale;
                let _ = window.set_position(tauri::PhysicalPosition::new(x as i32, y as i32));
            }
        }

        let _ = window.show();
        let _ = window.set_focus();
    }
}

async fn on_work_complete(app: &AppHandle, duration_mins: u64) {
    let (sound_enabled, notification_enabled) = read_preferences(app).await;

    if notification_enabled {
        let sender = crate::notify::TauriNotificationSender::new(app.clone());
        let _ = sender.send_focus_work_complete(duration_mins);
    }

    if sound_enabled {
        if let Some(audio) = app.try_state::<Arc<crate::focus_audio::FocusAudioManager>>() {
            audio.play(crate::focus_audio::FocusCue::WorkComplete, 0.8);
        }
    }
}

async fn on_break_complete(app: &AppHandle) {
    let (sound_enabled, notification_enabled) = read_preferences(app).await;

    if notification_enabled {
        let sender = crate::notify::TauriNotificationSender::new(app.clone());
        let _ = sender.send_focus_break_complete();
    }

    if sound_enabled {
        if let Some(audio) = app.try_state::<Arc<crate::focus_audio::FocusAudioManager>>() {
            audio.play(crate::focus_audio::FocusCue::BreakComplete, 0.8);
        }
    }

    open_tray_window(app);
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_as_str() {
        assert_eq!(
            Phase::Working {
                remaining: 10,
                total: 100
            }
            .as_str(),
            "working"
        );
        assert_eq!(
            Phase::BreakPending { remaining: 5 }.as_str(),
            "break_pending"
        );
        assert_eq!(
            Phase::Break {
                remaining: 10,
                total: 60
            }
            .as_str(),
            "break"
        );
    }

    #[test]
    fn phase_decrement() {
        let mut phase = Phase::Working {
            remaining: 10,
            total: 100,
        };
        phase.decrement();
        assert_eq!(phase.remaining(), 9);
        assert_eq!(phase.total(), 100);
    }

    #[test]
    fn phase_decrement_saturates_at_zero() {
        let mut phase = Phase::Working {
            remaining: 0,
            total: 100,
        };
        phase.decrement();
        assert_eq!(phase.remaining(), 0);
    }

    #[test]
    fn compute_break_short() {
        let config = FocusSessionConfig {
            work_secs: 1500,
            short_break_secs: 300,
            long_break_secs: 900,
            long_break_after: 4,
        };
        // Positions 0, 1, 2 → short break
        assert_eq!(compute_break_secs(&config, 0), 300);
        assert_eq!(compute_break_secs(&config, 1), 300);
        assert_eq!(compute_break_secs(&config, 2), 300);
    }

    #[test]
    fn compute_break_long() {
        let config = FocusSessionConfig {
            work_secs: 1500,
            short_break_secs: 300,
            long_break_secs: 900,
            long_break_after: 4,
        };
        // Position 3 → long break (3+1=4, 4%4=0)
        assert_eq!(compute_break_secs(&config, 3), 900);
    }

    #[test]
    fn is_long_break_boundary() {
        let config = FocusSessionConfig {
            work_secs: 1500,
            short_break_secs: 300,
            long_break_secs: 900,
            long_break_after: 4,
        };
        assert!(!is_long_break(0, &config));
        assert!(!is_long_break(1, &config));
        assert!(!is_long_break(2, &config));
        assert!(is_long_break(3, &config));
        assert!(!is_long_break(4, &config));
        assert!(is_long_break(7, &config));
    }

    #[test]
    fn build_sync_payload_fields() {
        let config = FocusSessionConfig {
            work_secs: 1500,
            short_break_secs: 300,
            long_break_secs: 900,
            long_break_after: 4,
        };
        let phase = Phase::Working {
            remaining: 1200,
            total: 1500,
        };
        let payload = build_sync_payload(&phase, 2, &config, false, Some("Deep Work"), true);
        assert_eq!(payload.phase, "working");
        assert_eq!(payload.remaining_secs, 1200);
        assert_eq!(payload.total_secs, 1500);
        assert_eq!(payload.cycle_position, 2);
        assert_eq!(payload.long_break_after, 4);
        assert!(!payload.paused);
        assert_eq!(payload.action_title.as_deref(), Some("Deep Work"));
        assert!(payload.dnd_active);
    }

    #[tokio::test]
    async fn timer_state_machine() {
        let timer = FocusTimer::new();
        assert!(timer.status().await.is_none());
    }

    #[tokio::test]
    async fn drift_correction_keeps_timing_accurate() {
        use std::time::{Duration, Instant};
        let start = Instant::now();
        let tick_duration = Duration::from_millis(100);
        for (tick_count, _) in (0..10).enumerate() {
            let next = start + tick_duration * ((tick_count as u32) + 1);
            tokio::time::sleep(next.saturating_duration_since(Instant::now())).await;
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed >= Duration::from_millis(950),
            "elapsed: {elapsed:?}"
        );
        assert!(
            elapsed < Duration::from_millis(1150),
            "elapsed: {elapsed:?}"
        );
    }
}
