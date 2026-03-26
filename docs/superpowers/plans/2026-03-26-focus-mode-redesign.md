# Focus Mode Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite Focus Mode with a backend-owned phase state machine, 5-second sync ticks, macOS DND integration, and a simplified reactive frontend.

**Architecture:** The desktop `FocusTimer` becomes a full phase state machine (Working → BreakPending → Break → Working cycle). The backend owns cycle position and break-type decisions. The frontend becomes a reactive listener with local 1-second countdown interpolation. A new `platform-macos::dnd` module provides macOS DND read/toggle via `defaults read` + `shortcuts run`.

**Tech Stack:** Rust (Tauri 2, tokio, serde), TypeScript (React, Tauri event API), macOS `defaults`/`shortcuts` CLI

**Spec:** `docs/superpowers/specs/2026-03-26-focus-mode-redesign.md`

---

## Task 1: New Event Constants & Payload Types (desktop-shared)

**Files:**
- Modify: `crates/desktop-shared/src/events.rs:72-76` (replace focus event constants)
- Modify: `crates/desktop-shared/src/events.rs:633-651` (replace focus payload structs)

- [ ] **Step 1: Replace focus event constants**

In `crates/desktop-shared/src/events.rs`, replace the old focus event constants:

```rust
// Replace these lines (72-76):
// pub const FOCUS_TICK: &str = "focus:tick";
// pub const FOCUS_COMPLETED: &str = "focus:completed";

// With:
pub const FOCUS_SYNC: &str = "focus:sync";
pub const FOCUS_PHASE_CHANGED: &str = "focus:phase_changed";
pub const FOCUS_WARNING: &str = "focus:warning";
pub const FOCUS_DND_UNAVAILABLE: &str = "focus:dnd_unavailable";
```

Keep `FOCUS_STATE_CHANGED`, `FOCUS_AUTO_DETECTED`, `FOCUS_AUTO_STARTED` unchanged.

- [ ] **Step 2: Replace focus payload structs**

In the same file, replace `FocusTickPayload` and `FocusCompletedPayload` with:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusSyncPayload {
    pub phase: String,
    pub remaining_secs: u64,
    pub total_secs: u64,
    pub cycle_position: u32,
    pub long_break_after: u32,
    pub paused: bool,
    pub action_title: Option<String>,
    pub dnd_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusWarningPayload {
    pub phase: String,
    pub remaining_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusDndUnavailablePayload {
    pub message: String,
}
```

- [ ] **Step 3: Update FocusTimerStatusResponse**

In `crates/desktop-shared/src/commands/productivity.rs`, replace `FocusTimerStatusResponse`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusSessionStatusResponse {
    pub active: bool,
    pub sync: Option<FocusSyncPayload>,
    pub session: Option<FocusSessionResponse>,
}
```

Add the import at the top of the file:

```rust
use crate::events::FocusSyncPayload;
```

- [ ] **Step 4: Build to verify types compile**

Run: `cargo build -p desktop-shared 2>&1 | tail -5`
Expected: Compilation errors in dependent crates (focus_timer.rs, productivity commands) — that's fine, we'll fix those in later tasks. The desktop-shared crate itself should compile clean.

- [ ] **Step 5: Commit**

```bash
git add crates/desktop-shared/
git commit -m "refactor(desktop-shared): replace focus tick/completed events with sync/phase_changed model"
```

---

## Task 2: macOS DND Module (platform-macos)

**Files:**
- Create: `crates/platform-macos/src/dnd.rs`
- Modify: `crates/platform-macos/src/lib.rs`

- [ ] **Step 1: Write tests for DND module**

Create `crates/platform-macos/src/dnd.rs`:

```rust
//! macOS Do Not Disturb — read state via `defaults read`, toggle via
//! `shortcuts run "Toggle Do Not Disturb"`.
//!
//! Both functions are no-ops on non-macOS platforms.

/// Check whether macOS Focus / Do Not Disturb is currently active.
///
/// Reads `com.apple.controlcenter` defaults. Returns `false` on non-macOS
/// or if the read fails (conservative: assume DND is off).
#[cfg(target_os = "macos")]
pub fn is_dnd_active() -> bool {
    // Implementation in step 3
    false
}

#[cfg(not(target_os = "macos"))]
pub fn is_dnd_active() -> bool {
    false
}

/// Toggle macOS Do Not Disturb via a Shortcuts automation.
///
/// Requires a user-created Shortcut named "Toggle Do Not Disturb".
/// Returns `Err` if the shortcut is not found or execution fails.
#[cfg(target_os = "macos")]
pub fn toggle_dnd() -> Result<(), String> {
    // Implementation in step 3
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn toggle_dnd() -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_dnd_active_returns_bool() {
        // Should not panic on any platform
        let _active = is_dnd_active();
    }

    #[test]
    fn toggle_dnd_returns_result() {
        // On CI / non-macOS this is a no-op Ok(())
        // On macOS without the shortcut, it returns Err — which is fine
        let _result = toggle_dnd();
    }
}
```

- [ ] **Step 2: Run tests to verify they pass (no-op implementations)**

Run: `cargo nextest run -p platform-macos 2>&1 | tail -10`
Expected: All tests pass (implementations are stubs/no-ops)

- [ ] **Step 3: Implement macOS-specific DND functions**

Replace the macOS `is_dnd_active` implementation:

```rust
#[cfg(target_os = "macos")]
pub fn is_dnd_active() -> bool {
    use std::process::Command;

    // macOS stores Focus/DND state in the assertionProperties of the
    // com.apple.ControlCenter defaults domain. A non-empty assertion
    // list indicates an active Focus mode.
    let output = Command::new("defaults")
        .args(["read", "com.apple.controlcenter", "NSStatusItem Visible FocusModes"])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            // "1" means the Focus indicator is visible (DND active)
            stdout.trim() == "1"
        }
        Err(_) => false,
    }
}
```

Replace the macOS `toggle_dnd` implementation:

```rust
#[cfg(target_os = "macos")]
pub fn toggle_dnd() -> Result<(), String> {
    use std::process::Command;

    let output = Command::new("shortcuts")
        .args(["run", "Toggle Do Not Disturb"])
        .output()
        .map_err(|e| format!("Failed to run shortcuts command: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "Shortcut 'Toggle Do Not Disturb' failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ))
    }
}
```

- [ ] **Step 4: Register the module**

In `crates/platform-macos/src/lib.rs`, add:

```rust
pub mod dnd;
```

- [ ] **Step 5: Build and run tests**

Run: `cargo nextest run -p platform-macos 2>&1 | tail -10`
Expected: All tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/platform-macos/
git commit -m "feat(platform-macos): add DND read/toggle via defaults + shortcuts"
```

---

## Task 3: Rewrite FocusTimer Backend State Machine

**Files:**
- Modify: `crates/desktop/src/focus_timer.rs` (full rewrite)

This is the core task — replacing the simple countdown with a phase state machine.

- [ ] **Step 1: Write the new FocusTimer with phase state machine**

Rewrite `crates/desktop/src/focus_timer.rs`:

```rust
//! Desktop focus timer — backend-owned phase state machine with 5-second sync
//! ticks, macOS DND integration, and automatic cycle progression.
//!
//! Phases: Working → BreakPending (5s) → Break → Working (auto-continues).
//! The backend owns cycle position and break-type decisions.

use std::sync::Arc;

use desktop_shared::commands::{FocusSessionResponse, FocusSessionStatusResponse};
use desktop_shared::events::{
    FocusDndUnavailablePayload, FocusSyncPayload, FocusWarningPayload, FOCUS_DND_UNAVAILABLE,
    FOCUS_PHASE_CHANGED, FOCUS_SYNC, FOCUS_WARNING,
};
use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use tracing::{debug, warn};

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
    Working { remaining: u64, total: u64 },
    BreakPending { remaining: u64 },
    Break { remaining: u64, total: u64 },
}

impl Phase {
    fn as_str(&self) -> &'static str {
        match self {
            Phase::Working { .. } => "working",
            Phase::BreakPending { .. } => "break_pending",
            Phase::Break { .. } => "break",
        }
    }

    fn remaining(&self) -> u64 {
        match self {
            Phase::Working { remaining, .. }
            | Phase::BreakPending { remaining }
            | Phase::Break { remaining, .. } => *remaining,
        }
    }

    fn total(&self) -> u64 {
        match self {
            Phase::Working { total, .. } | Phase::Break { total, .. } => *total,
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
    Stop,
}

// ── Session state (shared between timer loop and public API) ────────

struct SessionState {
    config: FocusSessionConfig,
    cycle_position: u32,
    dnd_enabled: bool,
    dnd_was_active_before: bool,
    sound_enabled: bool,
    notification_enabled: bool,
    action_title: Option<String>,
    action_id: Option<String>,
    handle: JoinHandle<()>,
    cmd_tx: mpsc::Sender<SessionCommand>,
}

// ── Public API ──────────────────────────────────────────────────────

pub struct FocusTimer {
    state: Mutex<Option<SessionState>>,
}

impl FocusTimer {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(None),
        }
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
        if guard.is_some() {
            return Err(
                common::ToolError::ExecutionFailed("Session already running".into()).into(),
            );
        }

        tray_countdown::FOCUS_ACTIVE.store(true, std::sync::atomic::Ordering::Relaxed);

        // DND: capture and enable
        let dnd_was_active_before = if dnd_enabled {
            let was = platform_macos::dnd::is_dnd_active();
            if !was {
                if let Err(e) = platform_macos::dnd::toggle_dnd() {
                    warn!("Failed to enable DND: {e}");
                    let _ = app.emit(
                        FOCUS_DND_UNAVAILABLE,
                        FocusDndUnavailablePayload {
                            message: format!(
                                "Could not enable Do Not Disturb: {e}. \
                                 Create a Shortcut named 'Toggle Do Not Disturb' to enable this."
                            ),
                        },
                    );
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
            state.handle.abort();
            let _ = state.handle.await;
            clear_tray_title(app);
            tray_countdown::notify_focus_ended(app);
            restore_dnd(&state);
            true
        } else {
            false
        }
    }

    pub async fn status(&self) -> Option<FocusSessionConfig> {
        let guard = self.state.lock().await;
        guard.as_ref().map(|s| s.config.clone())
    }

    pub async fn mark_completed(&self, app: &AppHandle) {
        let mut guard = self.state.lock().await;
        if let Some(state) = guard.take() {
            restore_dnd(&state);
            clear_tray_title(app);
            tray_countdown::notify_focus_ended(app);
        }
    }

    pub async fn preferences(&self) -> (bool, bool) {
        let guard = self.state.lock().await;
        guard
            .as_ref()
            .map(|s| (s.sound_enabled, s.notification_enabled))
            .unwrap_or((true, true))
    }
}

fn restore_dnd(state: &SessionState) {
    if state.dnd_enabled && !state.dnd_was_active_before {
        if let Err(e) = platform_macos::dnd::toggle_dnd() {
            warn!("Failed to restore DND: {e}");
        }
    }
}

// ── Session loop ─────────────────────────────────────────────────────

const WARNING_SECS: u64 = 30;
const BREAK_PENDING_SECS: u64 = 5;
const SYNC_INTERVAL: u64 = 5;

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
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));

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
                SessionCommand::Stop => {
                    // Caller handles cleanup via FocusTimer::stop()
                    return;
                }
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
                        start_focus_session(&app, &config, truncated_title.as_deref()).await;
                        phase = Phase::Working {
                            remaining: config.work_secs,
                            total: config.work_secs,
                        };
                        warning_shown = false;
                        sync_counter = 0;
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
            }
        }

        interval.tick().await;

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
                    start_focus_session(&app, &config, truncated_title.as_deref()).await;
                    phase = Phase::Working {
                        remaining: config.work_secs,
                        total: config.work_secs,
                    };
                    warning_shown = false;
                    sync_counter = 0;
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
            continue;
        }

        phase.decrement();

        // Warning at 30 seconds remaining (Working and Break phases only)
        if !warning_shown
            && phase.remaining() == WARNING_SECS
            && !matches!(phase, Phase::BreakPending { .. })
        {
            warning_shown = true;
            open_tray_window(&app);
            let _ = app.emit(
                FOCUS_WARNING,
                FocusWarningPayload {
                    phase: phase.as_str().to_string(),
                    remaining_secs: phase.remaining(),
                },
            );
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
            let _ = app.emit(
                FOCUS_SYNC,
                build_sync_payload(
                    &phase,
                    cycle_position,
                    &config,
                    paused,
                    truncated_title.as_deref(),
                    dnd_enabled,
                ),
            );
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

fn compute_break_secs(config: &FocusSessionConfig, cycle_position: u32) -> u64 {
    if is_long_break(cycle_position, config) {
        config.long_break_secs
    } else {
        config.short_break_secs
    }
}

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
        build_sync_payload(phase, cycle_position, config, paused, action_title, dnd_active),
    );
}

// ── AppCore delegates ────────────────────────────────────────────────

async fn start_focus_session(app: &AppHandle, config: &FocusSessionConfig, _action_title: Option<&str>) {
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

fn update_tray_title(app: &AppHandle, remaining_secs: u64, paused: bool, action_title: Option<&str>) {
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
    if let Some(window) = app.get_webview_window(WINDOW_TRAY) {
        let positioned = if let Some(tray) = app.tray_by_id("klynt-tray") {
            if let Ok(Some(rect)) = tray.rect() {
                if let Ok(win_size) = window.outer_size() {
                    let scale = window.scale_factor().unwrap_or(1.0);
                    let tray_pos = rect.position.to_physical::<f64>(scale);
                    let tray_size = rect.size.to_physical::<f64>(scale);
                    let x =
                        tray_pos.x + (tray_size.width / 2.0) - (win_size.width as f64 / 2.0);
                    let y = tray_pos.y + tray_size.height;
                    let _ =
                        window.set_position(tauri::PhysicalPosition::new(x as i32, y as i32));
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
        let body = format!("{duration_mins}m focus session complete. Break time!");
        let _ = crate::notify::TauriNotificationSender::new(app.clone())
            .send_sync("Focus Session Complete", &body);
    }

    #[cfg(target_os = "macos")]
    if sound_enabled {
        let _ = tokio::process::Command::new("afplay")
            .arg("/System/Library/Sounds/Glass.aiff")
            .spawn();
    }
    #[cfg(not(target_os = "macos"))]
    let _ = sound_enabled;
}

async fn on_break_complete(app: &AppHandle) {
    let (sound_enabled, notification_enabled) = read_preferences(app).await;

    if notification_enabled {
        let _ = crate::notify::TauriNotificationSender::new(app.clone())
            .send_sync("Break Over", "Ready for the next focus session!");
    }

    #[cfg(target_os = "macos")]
    if sound_enabled {
        let _ = tokio::process::Command::new("afplay")
            .arg("/System/Library/Sounds/Blow.aiff")
            .spawn();
    }
    #[cfg(not(target_os = "macos"))]
    let _ = sound_enabled;

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
        assert_eq!(Phase::BreakPending { remaining: 5 }.as_str(), "break_pending");
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
}
```

- [ ] **Step 2: Build to verify it compiles**

Run: `cargo build -p desktop 2>&1 | tail -20`
Expected: May have errors in `commands/productivity.rs` (old command signatures) — that's expected. The `focus_timer.rs` module itself should parse and compile.

- [ ] **Step 3: Run focus_timer tests**

Run: `cargo nextest run -p desktop -E 'test(focus_timer)' 2>&1 | tail -15`
Expected: All unit tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/src/focus_timer.rs
git commit -m "refactor(desktop): rewrite focus timer as backend-owned phase state machine"
```

---

## Task 4: Update Tauri Commands (desktop)

**Files:**
- Modify: `crates/desktop/src/commands/productivity.rs` (replace focus_timer_* commands)

- [ ] **Step 1: Replace focus timer commands**

In `crates/desktop/src/commands/productivity.rs`, replace the focus timer command section (lines 361-484). Remove these commands:
- `focus_timer_start`
- `focus_timer_stop`
- `focus_timer_status`
- `focus_break_start`
- `focus_timer_extend`
- `focus_timer_pause`
- `focus_timer_resume`

Replace with:

```rust
// ── Focus Session (tray-driven) ──────────────────────────────────────

use crate::focus_timer::{FocusSessionConfig, SessionCommand};
use desktop_shared::commands::FocusSessionStatusResponse;
use desktop_shared::events::FocusSyncPayload;

#[allow(clippy::too_many_arguments)]
#[tauri::command(rename_all = "snake_case")]
pub async fn focus_session_start(
    state: State<'_, Arc<AppCore>>,
    timer: State<'_, Arc<FocusTimer>>,
    app: tauri::AppHandle,
    work_secs: u64,
    short_break_secs: u64,
    long_break_secs: u64,
    long_break_after: u32,
    action_id: Option<String>,
    action_title: Option<String>,
    dnd_enabled: Option<bool>,
    sound_enabled: Option<bool>,
    notification_enabled: Option<bool>,
) -> Result<FocusSessionResponse, ApiError> {
    let config = FocusSessionConfig {
        work_secs,
        short_break_secs,
        long_break_secs,
        long_break_after,
    };

    // Start persistent session in AppCore first
    let session = state
        .productivity_focus_start(action_id.clone(), None, Some(work_secs as i64 / 60))
        .await?;

    // Start the desktop timer (phase state machine)
    timer
        .start(
            app,
            config,
            action_id,
            action_title,
            dnd_enabled.unwrap_or(false),
            sound_enabled.unwrap_or(true),
            notification_enabled.unwrap_or(true),
        )
        .await
        .map_err(|e| ApiError::new("TIMER_ERROR", e.to_string()))?;

    Ok(session)
}

#[tauri::command]
pub async fn focus_session_stop(
    state: State<'_, Arc<AppCore>>,
    timer: State<'_, Arc<FocusTimer>>,
    app: tauri::AppHandle,
    notes: Option<String>,
) -> Result<Option<FocusSessionResponse>, ApiError> {
    timer.stop(&app).await;
    // End whichever session is active (focus or break)
    let focus_result = state.productivity_focus_end(notes).await.unwrap_or(None);
    if focus_result.is_some() {
        return Ok(focus_result);
    }
    Ok(state.productivity_break_end().await.unwrap_or(None))
}

#[tauri::command]
pub async fn focus_session_status(
    state: State<'_, Arc<AppCore>>,
    timer: State<'_, Arc<FocusTimer>>,
) -> Result<FocusSessionStatusResponse, ApiError> {
    let session = state.productivity_focus_status().await?;
    let config = timer.status().await;

    Ok(FocusSessionStatusResponse {
        active: config.is_some(),
        sync: None, // Sync is pushed via events, not polled
        session,
    })
}

#[tauri::command]
pub async fn focus_session_pause(
    timer: State<'_, Arc<FocusTimer>>,
) -> Result<bool, ApiError> {
    Ok(timer.send_command(SessionCommand::Pause).await)
}

#[tauri::command]
pub async fn focus_session_resume(
    timer: State<'_, Arc<FocusTimer>>,
) -> Result<bool, ApiError> {
    Ok(timer.send_command(SessionCommand::Resume).await)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn focus_session_extend(
    timer: State<'_, Arc<FocusTimer>>,
    extra_secs: u64,
) -> Result<bool, ApiError> {
    Ok(timer.send_command(SessionCommand::Extend(extra_secs)).await)
}

#[tauri::command]
pub async fn focus_session_start_break(
    timer: State<'_, Arc<FocusTimer>>,
) -> Result<bool, ApiError> {
    Ok(timer.send_command(SessionCommand::StartBreak).await)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn focus_session_extend_work(
    timer: State<'_, Arc<FocusTimer>>,
    extra_mins: u64,
) -> Result<bool, ApiError> {
    Ok(timer
        .send_command(SessionCommand::ExtendWork(extra_mins * 60))
        .await)
}

#[tauri::command]
pub async fn focus_session_skip_break(
    timer: State<'_, Arc<FocusTimer>>,
) -> Result<bool, ApiError> {
    Ok(timer.send_command(SessionCommand::SkipBreak).await)
}
```

- [ ] **Step 2: Update DEV_COMMANDS and TAURI_ONLY**

In the same file, update `DEV_COMMANDS` at the bottom — remove old focus timer entries and add nothing (these are tray-only commands).

In `crates/desktop/src/dev_server/mod.rs`, update `TAURI_ONLY` to replace old command names:

```rust
const TAURI_ONLY: &[&str] = &[
    "permissions_check_accessibility",
    "permissions_open_accessibility",
    "resize_window",
    "open_url",
    "quit_app",
    "show_dashboard",
    "focus_session_start",
    "focus_session_stop",
    "focus_session_status",
    "focus_session_pause",
    "focus_session_resume",
    "focus_session_extend",
    "focus_session_start_break",
    "focus_session_extend_work",
    "focus_session_skip_break",
    "mcp_oauth_start",
    "mcp_oauth_disconnect",
];
```

- [ ] **Step 3: Update main.rs invoke_handler**

In `crates/desktop/src/main.rs`, in the `tauri::generate_handler![]` macro, replace the old focus command names with the new ones:

Replace:
```
commands::productivity::focus_timer_start,
commands::productivity::focus_timer_stop,
commands::productivity::focus_timer_status,
commands::productivity::focus_break_start,
commands::productivity::focus_timer_extend,
commands::productivity::focus_timer_pause,
commands::productivity::focus_timer_resume,
```

With:
```
commands::productivity::focus_session_start,
commands::productivity::focus_session_stop,
commands::productivity::focus_session_status,
commands::productivity::focus_session_pause,
commands::productivity::focus_session_resume,
commands::productivity::focus_session_extend,
commands::productivity::focus_session_start_break,
commands::productivity::focus_session_extend_work,
commands::productivity::focus_session_skip_break,
```

- [ ] **Step 4: Remove old FocusTimerStatusResponse import**

Update any import of `FocusTimerStatusResponse` to `FocusSessionStatusResponse` in `commands/productivity.rs`. Remove the old `use crate::focus_timer::TimerMode;` import.

- [ ] **Step 5: Also update distraction_respond to use new stop API**

In the `distraction_respond` command, the `"end_focus"` arm uses `timer.stop(&app).await;` — this still works because `FocusTimer::stop` has the same signature.

- [ ] **Step 6: Build the full desktop crate**

Run: `cargo build -p desktop 2>&1 | tail -20`
Expected: Clean compilation

- [ ] **Step 7: Run the dev_server parity test**

Run: `cargo nextest run -p desktop -E 'test(dev_server)' 2>&1 | tail -15`
Expected: Both parity tests pass (covers_all_tauri_commands, no_orphan_commands)

- [ ] **Step 8: Commit**

```bash
git add crates/desktop/
git commit -m "refactor(desktop): replace focus_timer commands with focus_session commands"
```

---

## Task 5: Update Frontend Types

**Files:**
- Modify: `desktop-ui/src/shared/types/productivity.ts`
- Modify: `desktop-ui/src/shared/types/index.ts`

- [ ] **Step 1: Replace focus types**

In `desktop-ui/src/shared/types/productivity.ts`, replace the Focus Timer section (lines 201-224):

```typescript
// ── Focus Session ─────────────────────────────────────────────

export interface FocusSyncPayload {
  phase: "working" | "break_pending" | "break" | "paused";
  remainingSecs: number;
  totalSecs: number;
  cyclePosition: number;
  longBreakAfter: number;
  paused: boolean;
  actionTitle: string | null;
  dndActive: boolean;
}

export interface FocusWarningPayload {
  phase: string;
  remainingSecs: number;
}

export interface FocusDndUnavailablePayload {
  message: string;
}

export interface FocusSessionStatus {
  active: boolean;
  sync: FocusSyncPayload | null;
  session: FocusSession | null;
}
```

- [ ] **Step 2: Update re-exports in index.ts**

In `desktop-ui/src/shared/types/index.ts`, replace the old focus type exports:

Replace:
```typescript
  FocusCompletedPayload,
  FocusTickPayload,
  FocusTimerStatus,
```

With:
```typescript
  FocusDndUnavailablePayload,
  FocusSessionStatus,
  FocusSyncPayload,
  FocusWarningPayload,
```

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/shared/types/
git commit -m "refactor(ui): update focus types for sync/phase_changed event model"
```

---

## Task 6: Rewrite useFocusTimer Hook

**Files:**
- Modify: `desktop-ui/src/shared/hooks/useFocusTimer.ts`

- [ ] **Step 1: Rewrite the hook**

Replace the entire content of `desktop-ui/src/shared/hooks/useFocusTimer.ts`:

```typescript
import { useEvent } from "@shared/hooks/useEvent";
import { useMutation } from "@shared/hooks/useMutation";
import { useQuery } from "@shared/hooks/useQuery";
import { todayISO } from "@shared/lib/dates";
import type {
  FocusDndUnavailablePayload,
  FocusSession,
  FocusSessionStatus,
  FocusSyncPayload,
  FocusWarningPayload,
} from "@shared/types";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

// ── Settings persistence ────────────────────────────────────────────

const SETTINGS_KEY = "klynt:focus:settings";

export interface FocusSettings {
  focusDuration: number; // work session (minutes)
  shortBreak: number; // short break (minutes)
  longBreak: number; // long break (minutes)
  longBreakAfter: number; // sessions before long break
  dndEnabled: boolean; // macOS Do Not Disturb
  soundEnabled: boolean; // play sound on completion
  notificationEnabled: boolean; // show OS notification on completion
}

const DEFAULT_SETTINGS: FocusSettings = {
  focusDuration: 25,
  shortBreak: 5,
  longBreak: 15,
  longBreakAfter: 4,
  dndEnabled: false,
  soundEnabled: true,
  notificationEnabled: true,
};

export interface FocusPreset {
  label: string;
  focusDuration: number;
  shortBreak: number;
}

export const FOCUS_PRESETS: FocusPreset[] = [
  { label: "Standard", focusDuration: 25, shortBreak: 5 },
  { label: "Deep Work", focusDuration: 50, shortBreak: 10 },
  { label: "Sprint", focusDuration: 15, shortBreak: 3 },
];

function loadSettings(): FocusSettings {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY);
    if (raw) return { ...DEFAULT_SETTINGS, ...JSON.parse(raw) };
  } catch {
    /* corrupted — fall through */
  }
  return { ...DEFAULT_SETTINGS };
}

function saveSettings(s: FocusSettings) {
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(s));
  } catch {
    /* localStorage may be unavailable */
  }
}

// ── Phase type ──────────────────────────────────────────────────────

export type FocusPhase = "idle" | "working" | "break_pending" | "break";

// ── Coaching intervention ────────────────────────────────────────────

interface CoachingIntervention {
  message: string;
  interventionType: string;
}

// ── Hook ────────────────────────────────────────────────────────────

export function useFocusTimer() {
  // Backend status (on mount / reconnect)
  const { data: initialStatus, refetch } = useQuery<FocusSessionStatus>(
    "focus_session_status",
    undefined,
    { active: false, sync: null, session: null },
  );

  // Mutations
  const startMut = useMutation<FocusSession, Record<string, unknown>>("focus_session_start");
  const stopMut = useMutation<FocusSession | null, { notes?: string }>("focus_session_stop");
  const pauseMut = useMutation<boolean, Record<string, never>>("focus_session_pause");
  const resumeMut = useMutation<boolean, Record<string, never>>("focus_session_resume");
  const extendMut = useMutation<boolean, { extra_secs: number }>("focus_session_extend");
  const startBreakMut = useMutation<boolean, Record<string, never>>("focus_session_start_break");
  const extendWorkMut = useMutation<boolean, { extra_mins: number }>("focus_session_extend_work");
  const skipBreakMut = useMutation<boolean, Record<string, never>>("focus_session_skip_break");
  const logDistractionMut = useMutation<void, { app_name: string }>("distraction_dismiss");

  // Today's completed sessions (for stats)
  const [todayDate] = useState(todayISO);
  const { data: todaySessions, refetch: refetchToday } = useQuery<FocusSession[]>(
    "productivity_sessions",
    { date: todayDate },
    [],
  );

  // Server state (updated by sync/phase_changed events)
  const [serverState, setServerState] = useState<FocusSyncPayload | null>(null);
  const [receivedAt, setReceivedAt] = useState<number>(0);
  const [settings, setSettings] = useState(loadSettings);
  const [selectedTask, setSelectedTask] = useState<{ id: string; title: string } | null>(null);
  const [coaching, setCoaching] = useState<CoachingIntervention | null>(null);
  const [showWarning, setShowWarning] = useState(false);
  const [dndHint, setDndHint] = useState<string | null>(null);

  // Local 1-second countdown
  const [localTick, setLocalTick] = useState(0);
  useEffect(() => {
    if (!serverState || serverState.paused) return;
    const id = setInterval(() => setLocalTick((t) => t + 1), 1000);
    return () => clearInterval(id);
  }, [serverState, serverState?.paused]);

  // Reset local tick when server state updates
  useEffect(() => {
    setLocalTick(0);
  }, [receivedAt]);

  // Sync event (every 5 seconds)
  useEvent<FocusSyncPayload>("focus:sync", (payload) => {
    if (payload) {
      setServerState(payload);
      setReceivedAt(Date.now());
      setShowWarning(false);
    }
  });

  // Phase changed (instant, on every transition)
  useEvent<FocusSyncPayload>("focus:phase_changed", (payload) => {
    if (payload) {
      setServerState(payload);
      setReceivedAt(Date.now());
      setShowWarning(false);
      refetchToday();
    }
  });

  // Warning (30 seconds remaining)
  useEvent<FocusWarningPayload>("focus:warning", (payload) => {
    if (payload) setShowWarning(true);
  });

  // DND unavailable hint
  useEvent<FocusDndUnavailablePayload>("focus:dnd_unavailable", (payload) => {
    if (payload?.message) setDndHint(payload.message);
  });

  // Coaching intervention after focus completion
  useEvent<CoachingIntervention>("coaching:intervention", (payload) => {
    if (payload?.message) setCoaching(payload);
  });

  // Sync from initial status on mount
  useEffect(() => {
    if (initialStatus.active && initialStatus.sync) {
      setServerState(initialStatus.sync);
      setReceivedAt(Date.now());
    }
  }, [initialStatus.active, initialStatus.sync]);

  // Derived state
  const phase: FocusPhase = serverState?.phase ?? "idle";
  const paused = serverState?.paused ?? false;
  const isActive = phase === "working" || phase === "break";

  // Local countdown interpolation
  const remainingSecs = useMemo(() => {
    if (!serverState || !isActive) return null;
    const elapsed = localTick;
    return Math.max(0, serverState.remainingSecs - elapsed);
  }, [serverState, isActive, localTick]);

  const totalSecs = serverState?.totalSecs ?? null;
  const cyclePosition = serverState?.cyclePosition ?? 0;
  const longBreakAfter = serverState?.longBreakAfter ?? settings.longBreakAfter;
  const actionTitle = serverState?.actionTitle ?? null;

  const updateSettings = useCallback((partial: Partial<FocusSettings>) => {
    setSettings((prev) => {
      const next = { ...prev, ...partial };
      saveSettings(next);
      return next;
    });
  }, []);

  // Today stats
  const { completedSessions, todayStats } = useMemo(() => {
    let sessions = 0;
    let totalMins = 0;
    let qualitySum = 0;
    let qualityCount = 0;
    for (const s of todaySessions) {
      if (!s.completed) continue;
      sessions++;
      totalMins += s.actualMins ?? 0;
      if (s.qualityScore != null) {
        qualitySum += s.qualityScore;
        qualityCount++;
      }
    }
    return {
      completedSessions: sessions,
      todayStats: {
        sessions,
        totalMins,
        avgQuality: qualityCount > 0 ? qualitySum / qualityCount : null,
      },
    };
  }, [todaySessions]);

  // Actions
  const start = useCallback(async () => {
    setCoaching(null);
    setShowWarning(false);
    setDndHint(null);
    await startMut.mutate({
      work_secs: settings.focusDuration * 60,
      short_break_secs: settings.shortBreak * 60,
      long_break_secs: settings.longBreak * 60,
      long_break_after: settings.longBreakAfter,
      action_id: selectedTask?.id,
      action_title: selectedTask?.title,
      dnd_enabled: settings.dndEnabled,
      sound_enabled: settings.soundEnabled,
      notification_enabled: settings.notificationEnabled,
    });
    refetch();
  }, [startMut, refetch, settings, selectedTask]);

  const stop = useCallback(
    async (notes?: string) => {
      await stopMut.mutate({ notes });
      setServerState(null);
      setSelectedTask(null);
      setShowWarning(false);
      setCoaching(null);
      refetch();
      refetchToday();
    },
    [stopMut, refetch, refetchToday],
  );

  const pause = useCallback(async () => {
    await pauseMut.mutate({});
  }, [pauseMut]);

  const resume = useCallback(async () => {
    await resumeMut.mutate({});
  }, [resumeMut]);

  const extend = useCallback(
    async (extraSecs: number) => {
      await extendMut.mutate({ extra_secs: extraSecs });
      setShowWarning(false);
    },
    [extendMut],
  );

  const startBreak = useCallback(async () => {
    await startBreakMut.mutate({});
  }, [startBreakMut]);

  const extendWork = useCallback(
    async (mins: number) => {
      await extendWorkMut.mutate({ extra_mins: mins });
    },
    [extendWorkMut],
  );

  const skipBreak = useCallback(async () => {
    await skipBreakMut.mutate({});
  }, [skipBreakMut]);

  const logDistraction = useCallback(
    async (category: string) => {
      await logDistractionMut.mutate({ app_name: category });
    },
    [logDistractionMut],
  );

  const activePreset = useMemo(
    () =>
      FOCUS_PRESETS.find(
        (p) => p.focusDuration === settings.focusDuration && p.shortBreak === settings.shortBreak,
      )?.label ?? "Custom",
    [settings.focusDuration, settings.shortBreak],
  );

  const applyPreset = useCallback(
    (preset: FocusPreset) => {
      updateSettings({ focusDuration: preset.focusDuration, shortBreak: preset.shortBreak });
    },
    [updateSettings],
  );

  return {
    // State
    phase,
    paused,
    active: isActive,
    remainingSecs,
    totalSecs,
    actionTitle,
    showWarning,
    dndHint,
    coaching,
    settings,
    completedSessions,
    cyclePosition,
    longBreakAfter,
    todayStats,
    activePreset,
    loading:
      startMut.loading ||
      stopMut.loading ||
      pauseMut.loading ||
      resumeMut.loading ||
      extendMut.loading ||
      startBreakMut.loading ||
      extendWorkMut.loading ||
      skipBreakMut.loading ||
      logDistractionMut.loading,

    // Actions
    start,
    stop,
    pause,
    resume,
    extend,
    startBreak,
    extendWork,
    skipBreak,
    logDistraction,
    updateSettings,
    applyPreset,
    dismissCoaching: useCallback(() => setCoaching(null), []),
    dismissDndHint: useCallback(() => setDndHint(null), []),
    selectTask: (id: string | null, title: string | null) => {
      setSelectedTask(id && title ? { id, title } : null);
    },
    selectedTaskId: selectedTask?.id ?? null,
    selectedTaskTitle: selectedTask?.title ?? null,
  };
}
```

- [ ] **Step 2: Verify types compile**

Run: `cd desktop-ui && bun run lint 2>&1 | tail -20`
Expected: May have errors in FocusControl.tsx (referencing old hook shape) — that's expected and will be fixed in the next task.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/shared/hooks/useFocusTimer.ts
git commit -m "refactor(ui): rewrite useFocusTimer as reactive listener with local countdown"
```

---

## Task 7: Adapt FocusControl Component

**Files:**
- Modify: `desktop-ui/src/features/tray/components/FocusControl.tsx`

- [ ] **Step 1: Update TimerView to use new hook shape**

The main changes to `FocusControl.tsx`:

1. Replace `phase === "focus"` checks with `phase === "working"`
2. Replace `phase === "break_pending"` — stays the same (backend now emits this)
3. Remove `completed` references — break_pending is now backend-driven
4. Use `timer.showWarning` instead of local warning calculation
5. Use `timer.cyclePosition` and `timer.longBreakAfter` for session dots
6. Add DND hint display
7. `startBreak` no longer needs breakMins — backend decides
8. `extendWork` takes minutes, not a complex start call
9. `skipBreak` is a simple command

Key replacements in the `TimerView` function:

Replace the destructuring:
```typescript
const {
  phase,
  paused,
  remainingSecs,
  totalSecs,
  settings,
  completedSessions,
  cyclePosition,
  longBreakAfter,
  loading,
  showWarning,
  dndHint,
} = timer;
```

Replace `isFocus` with `isWorking`:
```typescript
const isActive = phase === "working" || phase === "break";
const isBreak = phase === "break";
const isBreakPending = phase === "break_pending";
const isWorking = phase === "working";
```

Replace the warning check (delete the old local calculation):
```typescript
// showWarning comes from the hook now (backend sends focus:warning event)
```

Update cycle dots:
```typescript
const dotsCount = longBreakAfter;
const filledDots = cyclePosition;
```

Update phase label:
```typescript
const phaseLabel = (() => {
  if (paused) return "Paused";
  switch (phase) {
    case "working":
      return "Focus";
    case "break":
      return "Break";
    case "break_pending": {
      const cycleComplete = cyclePosition > 0 && cyclePosition % longBreakAfter === 0;
      return cycleComplete ? "Long Break" : "Break";
    }
    default:
      return "Focus";
  }
})();
```

Replace `QuickDistractionLog` condition:
```typescript
{isWorking && !showWarning && <QuickDistractionLog onLog={timer.logDistraction} />}
```

Replace `WarningBanner` condition:
```typescript
{showWarning && <WarningBanner timer={timer} isWorking={isWorking} />}
```

Update `WarningBanner` component — rename `isFocus` prop to `isWorking`:
```typescript
function WarningBanner({ timer, isWorking }: { timer: Timer; isWorking: boolean }) {
  const extendOptions = isWorking
    ? [
        { label: "+5m", secs: 300 },
        { label: "+10m", secs: 600 },
        { label: "+15m", secs: 900 },
      ]
    : [
        { label: "+30s", secs: 30 },
        { label: "+1m", secs: 60 },
        { label: "+2m", secs: 120 },
      ];
  // ... rest stays the same
```

Update `BreakPendingActions` — simplify since backend decides break type:
```typescript
function BreakPendingActions({ timer }: { timer: Timer }) {
  return (
    <div className="flex flex-col items-center gap-2 mt-3 animate-fade-in">
      <p className="text-[11px] text-muted-foreground font-light text-center">
        Break starting soon
      </p>

      <div className="flex gap-1.5">
        {[5, 10, 15].map((mins) => (
          <button
            key={mins}
            type="button"
            onClick={() => timer.extendWork(mins)}
            disabled={timer.loading}
            className="px-2 py-1.5 text-2xs rounded-full bg-accent text-muted-foreground font-light hover:bg-muted transition-colors disabled:opacity-50"
          >
            +{mins}m work
          </button>
        ))}
      </div>
      <div className="flex gap-1.5">
        <button
          type="button"
          onClick={timer.startBreak}
          disabled={timer.loading}
          className="flex items-center gap-1 px-3 py-1.5 rounded-full bg-muted text-2xs uppercase tracking-[0.1em] text-foreground font-light hover:bg-muted transition-colors disabled:opacity-50"
        >
          <Coffee className="size-3" strokeWidth={1.5} />
          Start Break
        </button>
        <button
          type="button"
          onClick={() => timer.stop()}
          disabled={timer.loading}
          className="flex items-center gap-1 px-3 py-1.5 rounded-full bg-accent text-2xs uppercase tracking-[0.1em] text-muted-foreground font-light hover:bg-muted transition-colors disabled:opacity-50"
        >
          <Square className="size-3" strokeWidth={1.5} />
          Stop
        </button>
      </div>
    </div>
  );
}
```

Replace the bottom controls for focus phase — replace `isFocus` with `isWorking`:
```typescript
) : isWorking ? (
  <>
    <PauseResumeButton timer={timer} />
    <button type="button" onClick={timer.skipBreak} ...>Break</button>
    ...
  </>
```

Wait — `takeBreak` in the old code stops focus early and goes to break. In the new model, this maps to `focus_session_stop` + the user starting a new session. Actually, looking at the state machine, the simplest approach is: the "Break" button during working should send `SessionCommand::StartBreak` after stopping the working phase early. But the backend state machine only accepts `StartBreak` during `BreakPending`.

For the working phase "Break" button, we should either:
1. Keep it as `stop()` (user exits session entirely), or
2. Add a `take_break` backend command

Since the spec says `take_break() → BreakPending(0s, immediate transition)` — this is already handled. The backend timer receives the command and transitions Working → BreakPending with 0 remaining, which immediately transitions to Break. We need a `TakeBreak` command.

Actually, looking back at the focus_timer.rs in Task 3 — I didn't add a `TakeBreak` command. Let me note this: we need to add `TakeBreak` to `SessionCommand` and handle it in the loop. The "Break" button during working sends `TakeBreak`, which sets the phase to `BreakPending { remaining: 0 }`.

Add to `SessionCommand` enum in Task 3's focus_timer.rs:
```rust
TakeBreak,
```

Add handler in the command drain loop:
```rust
SessionCommand::TakeBreak => {
    if matches!(phase, Phase::Working { .. }) {
        // End AppCore focus session
        end_focus_session(&app).await;
        on_work_complete(&app, (config.work_secs - phase.remaining()) / 60).await;
        phase = Phase::BreakPending { remaining: 0 };
        emit_phase_changed(...);
    }
}
```

And add the Tauri command:
```rust
#[tauri::command]
pub async fn focus_session_take_break(
    timer: State<'_, Arc<FocusTimer>>,
) -> Result<bool, ApiError> {
    Ok(timer.send_command(SessionCommand::TakeBreak).await)
}
```

Update the "Break" button in focus controls:
```typescript
<button type="button" onClick={timer.takeBreak} ...>Break</button>
```

And add `takeBreak` to the hook:
```typescript
const takeBreakMut = useMutation<boolean, Record<string, never>>("focus_session_take_break");
const takeBreak = useCallback(async () => {
  await takeBreakMut.mutate({});
}, [takeBreakMut]);
```

- [ ] **Step 2: Add DND hint display**

After the DND toggle, add:
```typescript
{dndHint && (
  <div className="flex items-center gap-2 mt-2 px-2">
    <p className="text-[10px] text-warning/70 font-light leading-tight flex-1">{dndHint}</p>
    <button
      type="button"
      onClick={timer.dismissDndHint}
      className="text-muted-foreground hover:text-foreground shrink-0"
    >
      <X className="size-3" />
    </button>
  </div>
)}
```

- [ ] **Step 3: Run lint**

Run: `cd desktop-ui && bun run lint:fix 2>&1 | tail -10`
Expected: Clean (Biome auto-fixes imports)

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/tray/
git commit -m "refactor(ui): adapt FocusControl for backend-owned phase state machine"
```

---

## Task 8: Add TakeBreak Command (missed in Task 3)

**Files:**
- Modify: `crates/desktop/src/focus_timer.rs` (add TakeBreak variant)
- Modify: `crates/desktop/src/commands/productivity.rs` (add Tauri command)
- Modify: `crates/desktop/src/main.rs` (register command)
- Modify: `crates/desktop/src/dev_server/mod.rs` (add to TAURI_ONLY)

- [ ] **Step 1: Add TakeBreak to SessionCommand**

In `focus_timer.rs`, add to the `SessionCommand` enum:

```rust
pub enum SessionCommand {
    Pause,
    Resume,
    Extend(u64),
    StartBreak,
    ExtendWork(u64),
    SkipBreak,
    TakeBreak,
    Stop,
}
```

- [ ] **Step 2: Handle TakeBreak in session_loop**

In the command drain loop in `session_loop`, add before the `SessionCommand::Stop` arm:

```rust
SessionCommand::TakeBreak => {
    if let Phase::Working { remaining, .. } = &phase {
        let worked_mins = (config.work_secs - remaining) / 60;
        end_focus_session(&app).await;
        on_work_complete(&app, worked_mins).await;
        // Immediate transition through break_pending
        phase = Phase::BreakPending { remaining: 0 };
        warning_shown = false;
        sync_counter = 0;
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
```

- [ ] **Step 3: Add Tauri command**

In `commands/productivity.rs`:

```rust
#[tauri::command]
pub async fn focus_session_take_break(
    timer: State<'_, Arc<FocusTimer>>,
) -> Result<bool, ApiError> {
    Ok(timer.send_command(SessionCommand::TakeBreak).await)
}
```

- [ ] **Step 4: Register in main.rs and TAURI_ONLY**

Add `commands::productivity::focus_session_take_break,` to `generate_handler![]` in main.rs.

Add `"focus_session_take_break"` to `TAURI_ONLY` in `dev_server/mod.rs`.

- [ ] **Step 5: Add takeBreak to the frontend hook**

In `useFocusTimer.ts`, add:

```typescript
const takeBreakMut = useMutation<boolean, Record<string, never>>("focus_session_take_break");

const takeBreak = useCallback(async () => {
  await takeBreakMut.mutate({});
}, [takeBreakMut]);
```

Add `takeBreakMut.loading` to the `loading` computation, and add `takeBreak` to the return object.

- [ ] **Step 6: Build and test**

Run: `cargo build -p desktop 2>&1 | tail -10`
Expected: Clean compilation

Run: `cargo nextest run -p desktop -E 'test(dev_server)' 2>&1 | tail -10`
Expected: Parity tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/desktop/ desktop-ui/src/shared/hooks/useFocusTimer.ts
git commit -m "feat(focus): add take_break command for early break transition"
```

---

## Task 9: Full Build, Lint, and Test Verification

**Files:** None (verification only)

- [ ] **Step 1: Workspace build**

Run: `cargo build --workspace 2>&1 | tail -10`
Expected: Clean compilation across all crates

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | tail -20`
Expected: 0 new warnings (pre-existing warnings in `cognitive` crate are OK)

- [ ] **Step 3: Rust tests**

Run: `cargo nextest run --workspace 2>&1 | tail -20`
Expected: All tests pass

- [ ] **Step 4: Frontend lint**

Run: `cd desktop-ui && bun run lint:fix 2>&1 | tail -10`
Expected: Clean

- [ ] **Step 5: Frontend types check**

Run: `cd desktop-ui && bunx tsc --noEmit 2>&1 | tail -20`
Expected: Clean (no type errors)

- [ ] **Step 6: Commit any lint fixes**

```bash
git add -A && git diff --cached --stat
# Only commit if there are changes
git commit -m "chore: lint fixes from focus mode redesign"
```

---

## Task 10: Final Commit and Summary

- [ ] **Step 1: Verify git log**

Run: `git log --oneline -10`
Expected: Clean commit history with conventional format commits from each task

- [ ] **Step 2: Verify no uncommitted changes**

Run: `git status`
Expected: Clean working tree
