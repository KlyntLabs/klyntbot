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
