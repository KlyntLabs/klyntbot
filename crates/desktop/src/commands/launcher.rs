//! Launcher IPC commands — thin Tauri delegates to `AppCore`.

use std::sync::Arc;

use desktop_macros::{klynt_command, klynt_raw_command};
use desktop_shared::{errors::ApiError, CommandResult};
use feature_focus::FocusMode;
use feature_launcher::{
    ClipboardEntry, DashboardData, LauncherExecuteResult, LauncherItem, Pin, ScriptRunner,
    SystemAction, SystemCommands, WindowAction,
};
use tauri::State;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn launcher_search(query: String) -> Vec<LauncherItem> {
    state.launcher_search(query).await
}

#[klynt_command]
pub async fn launcher_execute(item_id: String, kind: String) -> LauncherExecuteResult {
    state.launcher_execute(item_id, kind).await
}

#[klynt_command]
pub async fn launcher_dashboard() -> DashboardData {
    state.launcher_dashboard().await
}

#[klynt_command]
pub async fn launcher_pin(item_id: String, kind: String) -> () {
    state.launcher_pin(item_id, kind).await
}

#[klynt_command]
pub async fn launcher_unpin(item_id: String, kind: String) -> () {
    state.launcher_unpin(item_id, kind).await
}

#[klynt_command]
pub async fn launcher_list_pinned() -> Vec<Pin> {
    state.launcher_list_pinned().await
}

#[klynt_command]
pub async fn launcher_clipboard_paste(id: i64) -> Option<ClipboardEntry> {
    state.launcher_clipboard_paste(id).await
}

#[klynt_command]
pub async fn launcher_clipboard_delete(id: i64) -> () {
    state.launcher_clipboard_delete(id).await
}

#[klynt_command]
pub async fn launcher_clipboard_pin(id: i64, pinned: bool) -> () {
    state.launcher_clipboard_pin(id, pinned).await
}

#[klynt_command]
pub async fn launcher_rebuild_attention() -> u64 {
    state.launcher_rebuild_attention().await
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn launcher_run_script(
    path: String,
    args: Option<std::collections::HashMap<String, String>>,
) -> CommandResult<String> {
    let script_path = std::path::Path::new(&path);
    let args = args.unwrap_or_default();
    ScriptRunner::execute_with_args(script_path, &args)
        .await
        .map_err(|e| ApiError::new("SCRIPT_ERROR", e.to_string()))
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn launcher_system_command(
    state: State<'_, Arc<AppCore>>,
    action: SystemAction,
    args: Option<std::collections::HashMap<String, String>>,
) -> CommandResult<()> {
    let args = args.unwrap_or_default();
    let duration = args.get("duration").and_then(|s| parse_human_duration(s));

    if matches!(action, SystemAction::ToggleDoNotDisturb) && duration.is_some() {
        let ends_at = jiff::Timestamp::now()
            .checked_add(duration.unwrap())
            .map_err(|e| ApiError::new("BAD_DURATION", e.to_string()))?;
        let mgr = state
            .dnd_manager()
            .map_err(|e| ApiError::new("DND_ERROR", e.to_string()))?;
        mgr.activate(FocusMode::Dnd, ends_at)
            .await
            .map_err(|e| ApiError::new("FOCUS_ACTIVATE_ERROR", e.to_string()))?;
        return Ok(());
    }

    SystemCommands::execute(&action, duration)
        .await
        .map_err(|e| ApiError::new("SYSTEM_COMMAND_ERROR", e.to_string()))
}

fn parse_human_duration(s: &str) -> Option<std::time::Duration> {
    let s = s.trim();
    let last = s.chars().last()?;
    let (num_str, secs_per_unit) = if last.is_ascii_alphabetic() {
        let mult = match last.to_ascii_lowercase() {
            'm' => 60,
            'h' => 3600,
            'd' => 86400,
            _ => return None,
        };
        (&s[..s.len() - last.len_utf8()], mult)
    } else {
        (s, 60)
    };
    let num: u64 = num_str.parse().ok()?;
    Some(std::time::Duration::from_secs(num * secs_per_unit))
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn launcher_window_action(action: WindowAction) -> CommandResult<()> {
    feature_launcher::window_manager()
        .execute(&action)
        .map_err(|e| ApiError::new("WINDOW_ACTION_ERROR", e.to_string()))
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn launcher_open_app(path: String) -> CommandResult<()> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minutes() {
        assert_eq!(
            parse_human_duration("30m"),
            Some(std::time::Duration::from_secs(1800))
        );
    }
    #[test]
    fn parses_hours() {
        assert_eq!(
            parse_human_duration("2h"),
            Some(std::time::Duration::from_secs(7200))
        );
    }
    #[test]
    fn parses_days() {
        assert_eq!(
            parse_human_duration("1d"),
            Some(std::time::Duration::from_secs(86400))
        );
    }
    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_human_duration("xyz"), None);
    }
}

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
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
        "launcher_pin" => {
            let item_id = dev::get(body, "itemId").unwrap_or_default();
            let kind = dev::get(body, "kind").unwrap_or_default();
            dev::val(core.launcher_pin(item_id, kind).await)
        }
        "launcher_unpin" => {
            let item_id = dev::get(body, "itemId").unwrap_or_default();
            let kind = dev::get(body, "kind").unwrap_or_default();
            dev::val(core.launcher_unpin(item_id, kind).await)
        }
        "launcher_list_pinned" => dev::val(core.launcher_list_pinned().await),
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
            let args: Option<std::collections::HashMap<String, String>> = dev::get(body, "args");
            dev::val(launcher_run_script(path, args).await)
        }
        "launcher_system_command" => {
            let action: SystemAction = dev::get(body, "action").unwrap_or(SystemAction::LockScreen);
            let args: Option<std::collections::HashMap<String, String>> = dev::get(body, "args");
            let _ = core;
            let args = args.unwrap_or_default();
            let duration = args.get("duration").and_then(|s| parse_human_duration(s));
            dev::val(
                SystemCommands::execute(&action, duration)
                    .await
                    .map_err(|e| ApiError::new("SYSTEM_COMMAND_ERROR", e.to_string())),
            )
        }
        "launcher_window_action" => {
            let action: WindowAction = dev::get(body, "action").unwrap_or(WindowAction::Center);
            dev::val(launcher_window_action(action).await)
        }
        "launcher_open_app" => {
            let path: String = dev::get(body, "path").unwrap_or_default();
            dev::val(launcher_open_app(path).await)
        }
        _ => return None,
    })
}
