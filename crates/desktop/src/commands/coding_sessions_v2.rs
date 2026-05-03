use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;
use desktop_shared::{
    ClearMirrorCacheArgs, RewindResult, SessionExportArgs, SessionExportResult, SessionForkArgs,
    SessionForkResult, SessionRewindArgs,
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

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &::app_core::state::AppCore,
    body: &serde_json::Value,
) -> Option<desktop_shared::CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    // klynt_command sends `{ args: {...} }`; fall back to `body` for direct callers.
    fn parse_args<T: serde::de::DeserializeOwned>(body: &serde_json::Value) -> Result<T, ApiError> {
        let raw = body.get("args").cloned().unwrap_or_else(|| body.clone());
        serde_json::from_value(raw).map_err(|e| ApiError::new("VALIDATION", e.to_string()))
    }
    Some(match cmd {
        "coding_permissions_clear_mirror" => {
            let a: ClearMirrorCacheArgs = try_field!(parse_args(body));
            dev::val(
                core.coding_permissions_clear_mirror(a.tool, a.repo_id)
                    .await
                    .map_err(ApiError::from),
            )
        }
        "coding_sessions_rewind" => {
            let a: SessionRewindArgs = try_field!(parse_args(body));
            dev::val(
                core.coding_sessions_rewind(a.session_key, a.message_id)
                    .await
                    .map_err(ApiError::from),
            )
        }
        "coding_sessions_export" => {
            let a: SessionExportArgs = try_field!(parse_args(body));
            dev::val(
                core.coding_sessions_export(a.session_key, a.format)
                    .await
                    .map_err(ApiError::from),
            )
        }
        "coding_sessions_fork" => {
            let a: SessionForkArgs = try_field!(parse_args(body));
            dev::val(
                core.coding_sessions_fork(a.session_key, a.up_to_message)
                    .await
                    .map_err(ApiError::from),
            )
        }
        _ => return None,
    })
}
