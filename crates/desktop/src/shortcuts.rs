//! Shared global shortcut registration logic.
//!
//! Called at startup (from `setup` hook) and at runtime (from `shortcuts_update` command).
//! Uses `tauri-plugin-global-shortcut` runtime API for register/unregister.

use config::ShortcutsConfig;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use tauri::Emitter;

use crate::commands::window::{WINDOW_LAUNCHER, WINDOW_TRAY};
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
    let raw_mappings: [(&str, &'static str); 2] = [
        (&config.launcher, WINDOW_LAUNCHER),
        (&config.tray, WINDOW_TRAY),
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
///
/// Multi-monitor aware: if the window is visible but not focused (user is on
/// another monitor), move it to the cursor's monitor instead of hiding.
pub fn toggle_window(app: &AppHandle, window_label: &str) {
    if let Some(window) = crate::lazy_window::get_or_create_window(app, window_label) {
        let is_visible = window.is_visible().unwrap_or(false);
        let is_focused = window.is_focused().unwrap_or(false);

        if is_visible && is_focused {
            // Window is visible AND focused → user wants to dismiss it.
            let _ = window.hide();
        } else if window_label == WINDOW_TRAY {
            focus_timer::open_tray_window(app);
        } else {
            // Window is hidden OR visible-but-unfocused (on another monitor).
            // Position on the cursor's current monitor, then show + focus.
            position_on_cursor_monitor(&window);
            let _ = window.show();
            let _ = window.set_focus();
            // Emit so the frontend can focus the input — onFocusChanged may not
            // fire on the very first show (macOS has no prior focus state to diff).
            let _ = window.emit("window-shown", ());
        }
    }
}

/// Center a window on the monitor where the mouse cursor currently is.
/// Falls back to `window.center()` if cursor position can't be determined.
fn position_on_cursor_monitor(window: &tauri::WebviewWindow) {
    #[allow(unused_imports)]
    use tauri::Manager;

    // Get cursor position and find which monitor it's on.
    let cursor = match window.cursor_position() {
        Ok(pos) => pos,
        Err(_) => {
            let _ = window.center();
            return;
        }
    };

    let monitors = match window.available_monitors() {
        Ok(m) => m,
        Err(_) => {
            let _ = window.center();
            return;
        }
    };

    // Find the monitor containing the cursor.
    let target_monitor = monitors.iter().find(|m| {
        let pos = m.position();
        let size = m.size();
        let scale = m.scale_factor();
        let x = pos.x as f64 / scale;
        let y = pos.y as f64 / scale;
        let w = size.width as f64 / scale;
        let h = size.height as f64 / scale;
        cursor.x >= x && cursor.x < x + w && cursor.y >= y && cursor.y < y + h
    });

    if let Some(monitor) = target_monitor {
        let scale = monitor.scale_factor();
        let mon_pos: tauri::LogicalPosition<f64> = monitor.position().to_logical(scale);
        let mon_size: tauri::LogicalSize<f64> = monitor.size().to_logical(scale);
        let win_size = window
            .inner_size()
            .map(|s| {
                let ls: tauri::LogicalSize<f64> = s.to_logical(scale);
                ls
            })
            .unwrap_or(tauri::LogicalSize::new(660.0, 580.0));

        // Center the window on this monitor.
        let x = mon_pos.x + (mon_size.width - win_size.width) / 2.0;
        let y = mon_pos.y + (mon_size.height - win_size.height) / 3.0; // Slightly above center, like Spotlight
        let _ = window.set_position(tauri::LogicalPosition::new(x, y));
    } else {
        let _ = window.center();
    }
}
