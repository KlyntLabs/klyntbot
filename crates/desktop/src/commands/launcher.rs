//! Launcher IPC commands — thin Tauri delegates to `AppCore`.

use std::sync::Arc;

use desktop_shared::errors::ApiError;
use feature_launcher::{
    ClipboardEntry, DashboardData, LauncherExecuteResult, LauncherItem, ScriptRunner, SystemAction,
    SystemCommands,
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
    args: Option<std::collections::HashMap<String, String>>,
) -> Result<LauncherExecuteResult, ApiError> {
    state
        .launcher_execute(item_id, kind, args.unwrap_or_default())
        .await
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
pub async fn launcher_run_script(
    path: String,
    args: Option<std::collections::HashMap<String, String>>,
) -> Result<String, ApiError> {
    let script_path = std::path::Path::new(&path);
    let args = args.unwrap_or_default();
    // Args are passed as env vars (KLYNT_ARG_<UPPERCASE_NAME>=<value>) so the script
    // can reference them via $KLYNT_ARG_FOO without us rewriting the script file.
    // Template substitution into script content is deferred to Task 3.3 when
    // ScriptRunner gains # arg: front-matter parsing.
    ScriptRunner::execute_with_args(script_path, &args)
        .await
        .map_err(|e| ApiError::new("SCRIPT_ERROR", e.to_string()))
}

#[tauri::command]
pub async fn launcher_system_command(
    action: SystemAction,
    args: Option<std::collections::HashMap<String, String>>,
) -> Result<(), ApiError> {
    // args accepted here for IPC stability; DND duration threading deferred to Task 3.4
    // when SystemCommands::execute gains a duration parameter.
    let _ = args;
    SystemCommands::execute(&action)
        .await
        .map_err(|e| ApiError::new("SYSTEM_COMMAND_ERROR", e.to_string()))
}

#[tauri::command]
pub async fn launcher_open_app(path: String) -> Result<(), ApiError> {
    #[cfg(target_os = "macos")]
    {
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
            let args: std::collections::HashMap<String, String> =
                dev::get(body, "args").unwrap_or_default();
            dev::val(core.launcher_execute(item_id, kind, args).await)
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
            let args: Option<std::collections::HashMap<String, String>> =
                dev::get(body, "args");
            dev::val(launcher_run_script(path, args).await)
        }
        "launcher_system_command" => {
            let action: SystemAction = dev::get(body, "action").unwrap_or(SystemAction::LockScreen);
            let args: Option<std::collections::HashMap<String, String>> =
                dev::get(body, "args");
            dev::val(launcher_system_command(action, args).await)
        }
        "launcher_open_app" => {
            let path: String = dev::get(body, "path").unwrap_or_default();
            dev::val(launcher_open_app(path).await)
        }
        _ => return None,
    })
}
