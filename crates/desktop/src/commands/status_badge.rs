//! Mini status-badge window for no-view launcher executions.

use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::app_core::AppCore;
use desktop_macros::klynt_command;
use desktop_shared::{errors::ApiError, CommandResult};
use feature_launcher::BadgeKind;

const WIN_LABEL: &str = "status_badge";
const WIDTH: f64 = 280.0;
const HEIGHT: f64 = 40.0;

#[klynt_command]
pub async fn show_status_badge(
    app: AppHandle,
    text: String,
    kind: BadgeKind,
    duration_ms: Option<u32>,
) -> () {
    let dur = duration_ms.unwrap_or(2000);

    if let Some(existing) = app.get_webview_window(WIN_LABEL) {
        let _ = existing.emit(
            "badge:update",
            serde_json::json!({"text": text, "kind": kind, "ms": dur}),
        );
        return Ok(());
    }

    let url = WebviewUrl::App("status-badge.html".into());
    let win = WebviewWindowBuilder::new(&app, WIN_LABEL, url)
        .title("Klynt Status")
        .inner_size(WIDTH, HEIGHT)
        .decorations(false)
        .resizable(false)
        .always_on_top(true)
        .focused(false)
        .skip_taskbar(true)
        .transparent(true)
        .build()
        .map_err(|e| ApiError::new("BADGE_WINDOW", e.to_string()))?;

    // Position top-right of focused screen
    if let Some(monitor) = win.current_monitor().ok().flatten() {
        let size = monitor.size();
        let pos = monitor.position();
        let x = pos.x + size.width as i32 - (WIDTH as i32) - 24;
        let y = pos.y + 60;
        let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
    }

    let _ = win.emit(
        "badge:show",
        serde_json::json!({"text": text, "kind": kind, "ms": dur}),
    );

    let app_for_close = app.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(dur as u64)).await;
        if let Some(w) = app_for_close.get_webview_window(WIN_LABEL) {
            let _ = w.close();
        }
    });

    Ok(())
}

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    _core: &Arc<AppCore>,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
    use super::dev_helpers as dev;
    if cmd != "show_status_badge" {
        return None;
    }
    // Dev server can't open Tauri windows; treat as no-op success.
    let _ = body;
    Some(dev::val(Ok::<(), ApiError>(())))
}
