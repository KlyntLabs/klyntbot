use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;
use desktop_shared::{
    ClearMirrorCacheArgs, RewindResult, SessionExportArgs, SessionExportResult,
    SessionForkArgs, SessionForkResult, SessionRewindArgs,
};

#[klynt_command]
pub async fn coding_permissions_clear_mirror(args: ClearMirrorCacheArgs) -> u64 {
    state
        .coding_permissions_clear_mirror(args.tool, args.repo_id)
        .await
        .map_err(|e| ApiError::new("PERMISSIONS_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn coding_sessions_rewind(args: SessionRewindArgs) -> RewindResult {
    state
        .coding_sessions_rewind(args.session_key, args.message_id)
        .await
        .map_err(|e| ApiError::new("SESSIONS_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn coding_sessions_export(args: SessionExportArgs) -> SessionExportResult {
    state
        .coding_sessions_export(args.session_key, args.format)
        .await
        .map_err(|e| ApiError::new("SESSIONS_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn coding_sessions_fork(args: SessionForkArgs) -> SessionForkResult {
    state
        .coding_sessions_fork(args.session_key, args.up_to_message)
        .await
        .map_err(|e| ApiError::new("SESSIONS_ERROR", e.to_string()))
}
