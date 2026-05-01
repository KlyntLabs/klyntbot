use app_core::coding::doctor_handler::DiagnosticChecklist;
use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;

#[klynt_command]
pub async fn coding_doctor() -> DiagnosticChecklist {
    state
        .coding_doctor()
        .await
        .map_err(|e| ApiError::new("DOCTOR_ERROR", e.to_string()))
}
