use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;


#[klynt_command]
pub async fn coding_thread_metadata_generate(
    workspace_id: String,
    prompt: String,
) -> serde_json::Value {
    state
        .coding_thread_metadata_generate(&workspace_id, &prompt)
        .await
        .map(|r| serde_json::to_value(r).unwrap_or_default())
        .map_err(|e| ApiError::new("METADATA_ERROR", e.to_string()))
}

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &::app_core::state::AppCore,
    body: &serde_json::Value,
) -> Option<desktop_shared::CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "coding_thread_metadata_generate" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            let prompt = try_field!(dev::get_str(body, "prompt"));
            match core
                .coding_thread_metadata_generate(&workspace_id, &prompt)
                .await
            {
                Ok(r) => Ok(serde_json::to_value(r).unwrap_or_default()),
                Err(e) => Err(ApiError::new("METADATA_ERROR", e.to_string())),
            }
        }
        _ => return None,
    })
}
