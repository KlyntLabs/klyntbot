use app_core::coding::resume_handler::ResumeResult;
use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;

#[klynt_command]
pub async fn coding_resume(prefix: String) -> ResumeResult {
    state
        .coding_resume(&prefix)
        .await
        .map_err(|e| ApiError::new("RESUME_ERROR", e.to_string()))
}
