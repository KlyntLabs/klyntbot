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

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &::app_core::state::AppCore,
    body: &serde_json::Value,
) -> Option<desktop_shared::CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "coding_resume" => {
            let prefix = try_field!(dev::get_str(body, "prefix"));
            dev::val(core.coding_resume(&prefix).await.map_err(desktop_shared::errors::ApiError::from))
        }
        _ => return None,
    })
}
