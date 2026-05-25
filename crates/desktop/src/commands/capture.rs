use desktop_shared::commands::{CaptureStatusResponse, ShellHookStatusResponse};

use desktop_macros::klynt_command;

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
