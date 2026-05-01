use app_core::coding::doctor_handler::{DiagnosticChecklist, SandboxTestResult};
use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;

#[klynt_command]
pub async fn coding_doctor() -> DiagnosticChecklist {
    state
        .coding_doctor()
        .await
        .map_err(|e| ApiError::new("DOCTOR_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn coding_test_sandbox() -> SandboxTestResult {
    state
        .coding_test_sandbox()
        .await
        .map_err(|e| ApiError::new("SANDBOX_TEST_ERROR", e.to_string()))
}

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &::app_core::state::AppCore,
    _body: &serde_json::Value,
) -> Option<desktop_shared::CommandResult<serde_json::Value>> {
    use super::dev_helpers as dev;
    Some(match cmd {
        "coding_doctor" => dev::val(core.coding_doctor().await.map_err(desktop_shared::errors::ApiError::from)),
        "coding_test_sandbox" => dev::val(core.coding_test_sandbox().await.map_err(desktop_shared::errors::ApiError::from)),
        _ => return None,
    })
}
