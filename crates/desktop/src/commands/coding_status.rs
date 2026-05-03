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

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &::app_core::state::AppCore,
    body: &serde_json::Value,
) -> Option<desktop_shared::CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "coding_status" => {
            let session_key = try_field!(dev::get_str(body, "sessionKey"));
            dev::val(
                core.coding_status(&session_key)
                    .await
                    .map_err(desktop_shared::errors::ApiError::from),
            )
        }
        _ => return None,
    })
}
