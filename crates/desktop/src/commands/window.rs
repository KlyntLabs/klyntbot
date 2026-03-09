use tauri::Manager;

pub const WINDOW_TRAY: &str = "tray";
pub const WINDOW_LAUNCHER: &str = "launcher";

const RESIZABLE_WINDOWS: &[&str] = &[WINDOW_LAUNCHER, WINDOW_TRAY];

#[tauri::command]
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
pub fn open_url(url: String) {
    let _ = open::that(&url);
}

#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}
