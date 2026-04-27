//! Focus / DND Tauri commands — thin adapters to `DndManager` + the macOS bridge helpers.

use std::sync::Arc;

use desktop_shared::{errors::ApiError, CommandResult};
use desktop_macros::{klynt_command, klynt_raw_command};
use feature_focus::repo::FocusSession;
use feature_focus::FocusMode;
use tauri::State;

use crate::app_core::AppCore;

// ── Shortcut install helpers ──────────────────────────────────────────────────

/// Extract both bundled shortcuts to a temp dir and open them in Shortcuts.app
/// so the user is prompted to add them to their library.
///
/// Non-macOS: always returns an UNSUPPORTED error.
#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn focus_install_shortcuts() -> CommandResult<()> {
    #[cfg(target_os = "macos")]
    {
        use feature_focus::bridge::macos::install_bundled_shortcut;
        use feature_focus::{FOCUS_OFF_BYTES, FOCUS_ON_BYTES};
        install_bundled_shortcut("Klyntbot Focus On", FOCUS_ON_BYTES)
            .await
            .map_err(|e| ApiError::new("FOCUS_INSTALL_ERROR", e.to_string()))?;
        install_bundled_shortcut("Klyntbot Focus Off", FOCUS_OFF_BYTES)
            .await
            .map_err(|e| ApiError::new("FOCUS_INSTALL_ERROR", e.to_string()))?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    Err(ApiError::new(
        "UNSUPPORTED",
        "Focus shortcuts are only supported on macOS",
    ))
}

/// Check whether both bundled shortcuts are already installed in the user's
/// Shortcuts library. Returns `false` on non-macOS.
#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn focus_shortcuts_installed() -> CommandResult<bool> {
    #[cfg(target_os = "macos")]
    {
        use feature_focus::bridge::macos::is_shortcut_installed;
        let on = is_shortcut_installed("Klyntbot Focus On")
            .await
            .map_err(|e| ApiError::new("FOCUS_CHECK_ERROR", e.to_string()))?;
        let off = is_shortcut_installed("Klyntbot Focus Off")
            .await
            .map_err(|e| ApiError::new("FOCUS_CHECK_ERROR", e.to_string()))?;
        Ok(on && off)
    }
    #[cfg(not(target_os = "macos"))]
    Ok(false)
}

// ── Session commands ──────────────────────────────────────────────────────────

/// Activate a DND focus session ending at `ends_at` (RFC 3339).
#[klynt_command]
pub async fn focus_activate(
    mode: FocusMode,
    ends_at: String,
) -> FocusSession {
    let ts = ends_at
        .parse::<jiff::Timestamp>()
        .map_err(|e| ApiError::new("BAD_TIMESTAMP", e.to_string()))?;
    let mgr = state.dnd_manager()?;
    mgr.activate(mode, ts)
        .await
        .map_err(|e| ApiError::new("FOCUS_ACTIVATE_ERROR", e.to_string()))
}

/// Deactivate the active DND session for `mode`. Idempotent.
#[klynt_command]
pub async fn focus_deactivate(
    mode: FocusMode,
) -> () {
    let mgr = state.dnd_manager()?;
    mgr.deactivate(mode)
        .await
        .map_err(|e| ApiError::new("FOCUS_DEACTIVATE_ERROR", e.to_string()))
}

/// Extend the active session for `mode`, setting a new end time (RFC 3339).
#[klynt_command]
pub async fn focus_extend(
    mode: FocusMode,
    new_ends_at: String,
) -> FocusSession {
    let ts = new_ends_at
        .parse::<jiff::Timestamp>()
        .map_err(|e| ApiError::new("BAD_TIMESTAMP", e.to_string()))?;
    let mgr = state.dnd_manager()?;
    mgr.extend(mode, ts)
        .await
        .map_err(|e| ApiError::new("FOCUS_EXTEND_ERROR", e.to_string()))
}

/// Return the current active session for `mode`, or `null` if none.
#[klynt_command]
pub async fn focus_active(
    mode: FocusMode,
) -> Option<FocusSession> {
    let mgr = state.dnd_manager()?;
    mgr.active(mode)
        .await
        .map_err(|e| ApiError::new("FOCUS_ACTIVE_ERROR", e.to_string()))
}

// ── Dev-server coverage ───────────────────────────────────────────────────────

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
    use super::dev_helpers as dev;

    Some(match cmd {
        "focus_install_shortcuts" => dev::val(focus_install_shortcuts().await),
        "focus_shortcuts_installed" => dev::val(focus_shortcuts_installed().await),
        "focus_activate" => {
            let mode: FocusMode = dev::get(body, "mode").unwrap_or(FocusMode::Dnd);
            let ends_at: String = dev::get(body, "endsAt").unwrap_or_default();
            dev::val(
                async {
                    let ts = ends_at
                        .parse::<jiff::Timestamp>()
                        .map_err(|e| ApiError::new("BAD_TIMESTAMP", e.to_string()))?;
                    let mgr = core.dnd_manager()?;
                    mgr.activate(mode, ts)
                        .await
                        .map_err(|e| ApiError::new("FOCUS_ACTIVATE_ERROR", e.to_string()))
                }
                .await,
            )
        }
        "focus_deactivate" => {
            let mode: FocusMode = dev::get(body, "mode").unwrap_or(FocusMode::Dnd);
            dev::val(
                async {
                    let mgr = core.dnd_manager()?;
                    mgr.deactivate(mode)
                        .await
                        .map_err(|e| ApiError::new("FOCUS_DEACTIVATE_ERROR", e.to_string()))
                }
                .await,
            )
        }
        "focus_extend" => {
            let mode: FocusMode = dev::get(body, "mode").unwrap_or(FocusMode::Dnd);
            let new_ends_at: String = dev::get(body, "newEndsAt").unwrap_or_default();
            dev::val(
                async {
                    let ts = new_ends_at
                        .parse::<jiff::Timestamp>()
                        .map_err(|e| ApiError::new("BAD_TIMESTAMP", e.to_string()))?;
                    let mgr = core.dnd_manager()?;
                    mgr.extend(mode, ts)
                        .await
                        .map_err(|e| ApiError::new("FOCUS_EXTEND_ERROR", e.to_string()))
                }
                .await,
            )
        }
        "focus_active" => {
            let mode: FocusMode = dev::get(body, "mode").unwrap_or(FocusMode::Dnd);
            dev::val(
                async {
                    let mgr = core.dnd_manager()?;
                    mgr.active(mode)
                        .await
                        .map_err(|e| ApiError::new("FOCUS_ACTIVE_ERROR", e.to_string()))
                }
                .await,
            )
        }
        _ => return None,
    })
}
