use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;

#[klynt_command]
pub async fn coding_sessions_star(session_key: String) -> () {
    state
        .coding_sessions_star(&session_key)
        .await
        .map_err(|e| ApiError::new("SESSIONS_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn coding_sessions_unstar(session_key: String) -> () {
    state
        .coding_sessions_unstar(&session_key)
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
    Some(match cmd {
        "coding_sessions_star" => {
            let session_key = try_field!(dev::get_str(body, "sessionKey"));
            dev::val(core.coding_sessions_star(&session_key).await.map_err(desktop_shared::errors::ApiError::from))
        }
        "coding_sessions_unstar" => {
            let session_key = try_field!(dev::get_str(body, "sessionKey"));
            dev::val(core.coding_sessions_unstar(&session_key).await.map_err(desktop_shared::errors::ApiError::from))
        }
        _ => return None,
    })
}
