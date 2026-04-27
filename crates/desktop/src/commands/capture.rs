use desktop_shared::commands::{CaptureStatusResponse, ShellHookStatusResponse};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
#[specta::specta]
pub async fn capture_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<CaptureStatusResponse, ApiError> {
    state.get_capture_status().await
}

#[tauri::command]
#[specta::specta]
pub async fn capture_shell_hook_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<ShellHookStatusResponse, ApiError> {
    state.get_shell_hook_status().await
}

#[tauri::command]
#[specta::specta]
pub async fn capture_install_shell_hook(
    state: State<'_, Arc<AppCore>>,
) -> Result<String, ApiError> {
    state.install_shell_hook().await
}

#[tauri::command]
#[specta::specta]
pub async fn capture_uninstall_shell_hook(
    state: State<'_, Arc<AppCore>>,
) -> Result<String, ApiError> {
    state.uninstall_shell_hook().await
}

#[tauri::command]
#[specta::specta]
pub async fn capture_get_ingestion_token(
    state: State<'_, Arc<AppCore>>,
) -> Result<String, ApiError> {
    state.get_ingestion_token().await
}

#[tauri::command]
#[specta::specta]
pub async fn capture_regenerate_ingestion_token(
    state: State<'_, Arc<AppCore>>,
) -> Result<String, ApiError> {
    state.regenerate_ingestion_token().await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "capture_status",
    "capture_shell_hook_status",
    "capture_install_shell_hook",
    "capture_uninstall_shell_hook",
    "capture_get_ingestion_token",
    "capture_regenerate_ingestion_token",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    _body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers as dev;
    Some(match cmd {
        "capture_status" => dev::val(core.get_capture_status().await),
        "capture_shell_hook_status" => dev::val(core.get_shell_hook_status().await),
        "capture_install_shell_hook" => dev::val(core.install_shell_hook().await),
        "capture_uninstall_shell_hook" => dev::val(core.uninstall_shell_hook().await),
        "capture_get_ingestion_token" => dev::val(core.get_ingestion_token().await),
        "capture_regenerate_ingestion_token" => dev::val(core.regenerate_ingestion_token().await),
        _ => return None,
    })
}
