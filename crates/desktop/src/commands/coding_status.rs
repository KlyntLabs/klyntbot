use app_core::coding::status_handler::CodingStatus;
use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;

#[klynt_command]
pub async fn coding_status(session_key: String) -> CodingStatus {
    state
        .coding_status(&session_key)
        .await
        .map_err(|e| ApiError::new("STATUS_ERROR", e.to_string()))
}
