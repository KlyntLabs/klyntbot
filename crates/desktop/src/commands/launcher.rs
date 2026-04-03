//! Launcher IPC commands — thin Tauri delegates to `AppCore`.

use std::sync::Arc;

use desktop_shared::errors::ApiError;
use feature_launcher::{
    ClipboardEntry, DashboardData, LauncherItem, ScriptRunner, SystemAction, SystemCommands,
    WindowAction,
};
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn launcher_search(
    state: State<'_, Arc<AppCore>>,
    query: String,
) -> Result<Vec<LauncherItem>, ApiError> {
    state.launcher_search(query).await
}

#[tauri::command]
pub async fn launcher_execute(
    state: State<'_, Arc<AppCore>>,
    item_id: String,
    kind: String,
) -> Result<(), ApiError> {
    state.launcher_execute(item_id, kind).await
}

#[tauri::command]
pub async fn launcher_dashboard(state: State<'_, Arc<AppCore>>) -> Result<DashboardData, ApiError> {
    state.launcher_dashboard().await
}

#[tauri::command]
pub async fn launcher_clipboard_paste(
    state: State<'_, Arc<AppCore>>,
    id: i64,
) -> Result<Option<ClipboardEntry>, ApiError> {
    state.launcher_clipboard_paste(id).await
}

#[tauri::command]
pub async fn launcher_clipboard_delete(
    state: State<'_, Arc<AppCore>>,
    id: i64,
) -> Result<(), ApiError> {
    state.launcher_clipboard_delete(id).await
}

#[tauri::command]
pub async fn launcher_clipboard_pin(
    state: State<'_, Arc<AppCore>>,
    id: i64,
    pinned: bool,
) -> Result<(), ApiError> {
    state.launcher_clipboard_pin(id, pinned).await
}

#[tauri::command]
pub async fn launcher_run_script(path: String) -> Result<String, ApiError> {
    let script_path = std::path::Path::new(&path);
    ScriptRunner::execute(script_path)
        .await
        .map_err(|e| ApiError::new("SCRIPT_ERROR", e.to_string()))
}

#[tauri::command]
pub async fn launcher_system_command(action: SystemAction) -> Result<(), ApiError> {
    SystemCommands::execute(&action)
        .await
        .map_err(|e| ApiError::new("SYSTEM_COMMAND_ERROR", e.to_string()))
}

#[tauri::command]
pub async fn launcher_window_action(
    state: State<'_, Arc<AppCore>>,
    action: WindowAction,
) -> Result<(), ApiError> {
    state.launcher_window_action(action).await
}

#[tauri::command]
pub async fn launcher_open_app(path: String) -> Result<(), ApiError> {
    #[cfg(target_os = "macos")]
    {
        // Activate in-place if already running — avoids ~200ms `open` subprocess latency
        let app_path = std::path::PathBuf::from(&path);
        let running = platform_macos::apps::running_applications();
        let already_running = running
            .iter()
            .find(|a| a.path.as_ref().is_some_and(|p| p == &app_path));
        if let Some(app) = already_running {
            if platform_macos::apps::activate_app(app.pid) {
                return Ok(());
            }
        }
        // Fall back to `open` for non-running apps
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| ApiError::new("OPEN_APP_ERROR", e.to_string()))?;
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        return Err(ApiError::new(
            "UNSUPPORTED",
            "App launching only supported on macOS",
        ));
    }
    Ok(())
}

#[allow(dead_code)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "launcher_search",
    "launcher_execute",
    "launcher_dashboard",
    "launcher_clipboard_paste",
    "launcher_clipboard_delete",
    "launcher_clipboard_pin",
    "launcher_run_script",
    "launcher_system_command",
    "launcher_open_app",
    "launcher_window_action",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers as dev;

    Some(match cmd {
        "launcher_search" => {
            let query = dev::get(body, "query").unwrap_or_default();
            dev::val(core.launcher_search(query).await)
        }
        "launcher_execute" => {
            let item_id = dev::get(body, "itemId").unwrap_or_default();
            let kind = dev::get(body, "kind").unwrap_or_default();
            dev::val(core.launcher_execute(item_id, kind).await)
        }
        "launcher_dashboard" => dev::val(core.launcher_dashboard().await),
        "launcher_clipboard_paste" => {
            let id: i64 = dev::get(body, "id").unwrap_or_default();
            dev::val(core.launcher_clipboard_paste(id).await)
        }
        "launcher_clipboard_delete" => {
            let id: i64 = dev::get(body, "id").unwrap_or_default();
            dev::val(core.launcher_clipboard_delete(id).await)
        }
        "launcher_clipboard_pin" => {
            let id: i64 = dev::get(body, "id").unwrap_or_default();
            let pinned: bool = dev::get(body, "pinned").unwrap_or_default();
            dev::val(core.launcher_clipboard_pin(id, pinned).await)
        }
        "launcher_run_script" => {
            let path: String = dev::get(body, "path").unwrap_or_default();
            dev::val(launcher_run_script(path).await)
        }
        "launcher_system_command" => {
            let action: SystemAction = dev::get(body, "action").unwrap_or(SystemAction::LockScreen);
            dev::val(launcher_system_command(action).await)
        }
        "launcher_open_app" => {
            let path: String = dev::get(body, "path").unwrap_or_default();
            dev::val(launcher_open_app(path).await)
        }
        "launcher_window_action" => {
            let action: feature_launcher::WindowAction =
                dev::get(body, "action").unwrap_or(feature_launcher::WindowAction::Maximize);
            dev::val(core.launcher_window_action(action).await)
        }
        _ => return None,
    })
}
