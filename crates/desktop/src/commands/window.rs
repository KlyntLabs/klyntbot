use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use app_core::AppCore;
use tauri::Manager;

pub const WINDOW_TRAY: &str = "tray";
pub const WINDOW_LAUNCHER: &str = "launcher";

const RESIZABLE_WINDOWS: &[&str] = &[WINDOW_LAUNCHER, WINDOW_TRAY];

/// Set to `true` when the user explicitly requests a quit (via `quit_app`).
/// The `RunEvent::ExitRequested` handler checks this flag to distinguish
/// real quit intent from close-button presses (which should only hide windows).
pub static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);

#[tauri::command]
#[specta::specta]
pub fn resize_window(app: tauri::AppHandle, label: String, height: f64) {
    if !RESIZABLE_WINDOWS.contains(&label.as_str()) {
        return;
    }
    if let Some(window) = app.get_webview_window(&label) {
        let width = window
            .outer_size()
            .map(|s| {
                let scale = window.scale_factor().unwrap_or(1.0);
                s.width as f64 / scale
            })
            .unwrap_or(320.0);
        let _ = window.set_size(tauri::LogicalSize::new(width, height));
    }
}

#[tauri::command]
#[specta::specta]
pub fn open_url(url: String) {
    let _ = open::that(&url);
}

#[tauri::command]
#[specta::specta]
pub fn show_dashboard(app: tauri::AppHandle) {
    // Restore Dock icon before showing the window
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
#[specta::specta]
pub async fn quit_app(app: tauri::AppHandle) {
    QUIT_REQUESTED.store(true, Ordering::SeqCst);
    if let Some(core) = app.try_state::<Arc<AppCore>>() {
        core.shutdown().await;
    }
    app.exit(0);
}
