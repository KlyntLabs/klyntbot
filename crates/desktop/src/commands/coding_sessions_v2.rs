use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;
use desktop_shared::{ExportFormat, RewindResult, SessionExportResult, SessionForkResult};

#[klynt_command]
pub async fn coding_permissions_clear_mirror(tool: String, repo_id: Option<String>) -> u64 {
    state
        .coding_permissions_clear_mirror(tool, repo_id)
        .await
        .map_err(|e| ApiError::new("PERMISSIONS_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn coding_sessions_rewind(session_key: String, message_id: String) -> RewindResult {
    state
        .coding_sessions_rewind(session_key, message_id)
        .await
        .map_err(|e| ApiError::new("SESSIONS_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn coding_sessions_export(session_key: String, format: ExportFormat) -> SessionExportResult {
    state
        .coding_sessions_export(session_key, format)
        .await
        .map_err(|e| ApiError::new("SESSIONS_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn coding_sessions_fork(session_key: String, up_to_message: Option<String>) -> SessionForkResult {
    state
        .coding_sessions_fork(session_key, up_to_message)
        .await
        .map_err(|e| ApiError::new("SESSIONS_ERROR", e.to_string()))
}
