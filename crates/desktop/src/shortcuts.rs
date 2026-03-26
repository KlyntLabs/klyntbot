//! Shared global shortcut registration logic.
//!
//! Called at startup (from `setup` hook) and at runtime (from `shortcuts_update` command).
//! Uses `tauri-plugin-global-shortcut` runtime API for register/unregister.

use config::ShortcutsConfig;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use tauri::Emitter;

use crate::commands::window::{WINDOW_LAUNCHER, WINDOW_QUICK_CAPTURE, WINDOW_TRAY};
use crate::focus_timer;

/// Register all three global shortcuts from config, mapping each to its window toggle.
///
/// Unregisters all existing shortcuts first (clean slate). If any shortcut string
/// is invalid or the OS rejects registration, returns an error describing the failure.
pub fn register_shortcuts(app: &AppHandle, config: &ShortcutsConfig) -> Result<(), String> {
    let manager = app.global_shortcut();

    // Unregister all existing shortcuts (idempotent — safe if none registered).
    manager
        .unregister_all()
        .map_err(|e| format!("failed to unregister existing shortcuts: {e}"))?;

    // Phase 1: Parse all shortcut strings — fail fast before touching OS state.
    let raw_mappings: [(&str, &'static str); 3] = [
        (&config.launcher, WINDOW_LAUNCHER),
        (&config.tray, WINDOW_TRAY),
        (&config.quick_capture, WINDOW_QUICK_CAPTURE),
    ];
    let parsed: Vec<(tauri_plugin_global_shortcut::Shortcut, &'static str)> = raw_mappings
        .iter()
        .map(|(s, label)| {
            s.parse::<tauri_plugin_global_shortcut::Shortcut>()
                .map(|shortcut| (shortcut, *label))
                .map_err(|e| format!("invalid shortcut '{s}': {e}"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Phase 2: Register each parsed shortcut with its window toggle handler.
    for (shortcut, window_label) in parsed {
        let app_clone = app.clone();
        manager
            .on_shortcut(shortcut, move |_app, _shortcut, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }
                toggle_window(&app_clone, window_label);
            })
            .map_err(|e| format!("failed to register shortcut: {e}"))?;
    }

    Ok(())
}

/// Toggle a window's visibility. Tray uses `focus_timer::open_tray_window` for
/// correct positioning; others use center + show + focus.
pub fn toggle_window(app: &AppHandle, window_label: &str) {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window(window_label) {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else if window_label == WINDOW_TRAY {
            focus_timer::open_tray_window(app);
        } else {
            let _ = window.center();
            let _ = window.show();
            let _ = window.set_focus();
            // Emit so the frontend can focus the input — onFocusChanged may not
            // fire on the very first show (macOS has no prior focus state to diff).
            let _ = window.emit("window-shown", ());
        }
    }
}
