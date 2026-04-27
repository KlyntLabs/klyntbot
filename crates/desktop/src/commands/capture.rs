use desktop_shared::commands::{CaptureStatusResponse, ShellHookStatusResponse};
use desktop_shared::CommandResult;

use desktop_macros::klynt_command;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn capture_status() -> CaptureStatusResponse {
    state.get_capture_status().await
}

#[klynt_command]
pub async fn capture_shell_hook_status() -> ShellHookStatusResponse {
    state.get_shell_hook_status().await
}

#[klynt_command]
pub async fn capture_install_shell_hook() -> String {
    state.install_shell_hook().await
}

#[klynt_command]
pub async fn capture_uninstall_shell_hook() -> String {
    state.uninstall_shell_hook().await
}

#[klynt_command]
pub async fn capture_get_ingestion_token() -> String {
    state.get_ingestion_token().await
}

#[klynt_command]
pub async fn capture_regenerate_ingestion_token() -> String {
    state.regenerate_ingestion_token().await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    _body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
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
