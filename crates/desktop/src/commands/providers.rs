use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;

#[klynt_command]
pub async fn providers_list() -> serde_json::Value {
    state
        .providers_list()
        .await
        .map(|r| serde_json::to_value(r).unwrap_or_default())
        .map_err(|e| ApiError::new("PROVIDER_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn provider_status(provider_id: String) -> serde_json::Value {
    state
        .provider_status(&provider_id)
        .await
        .map(|r| serde_json::to_value(r).unwrap_or_default())
        .map_err(|e| ApiError::new("PROVIDER_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn model_list(workspace_id: String) -> serde_json::Value {
    state
        .model_list(&workspace_id)
        .await
        .map(|r| serde_json::to_value(r).unwrap_or_default())
        .map_err(|e| ApiError::new("PROVIDER_ERROR", e.to_string()))
}

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &::app_core::state::AppCore,
    body: &serde_json::Value,
) -> Option<desktop_shared::CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "providers_list" => match core.providers_list().await {
            Ok(r) => Ok(serde_json::to_value(r).unwrap_or_default()),
            Err(e) => Err(ApiError::new("PROVIDER_ERROR", e.to_string())),
        },
        "provider_status" => {
            let provider_id = try_field!(dev::get_str(body, "providerId"));
            match core.provider_status(&provider_id).await {
                Ok(r) => Ok(serde_json::to_value(r).unwrap_or_default()),
                Err(e) => Err(ApiError::new("PROVIDER_ERROR", e.to_string())),
            }
        }
        "model_list" => {
            // workspaceId is optional in browser dev; downstream
            // handler ignores it today.
            let workspace_id = dev::get_str(body, "workspaceId").unwrap_or_default();
            match core.model_list(&workspace_id).await {
                Ok(r) => Ok(serde_json::to_value(r).unwrap_or_default()),
                Err(e) => Err(ApiError::new("PROVIDER_ERROR", e.to_string())),
            }
        }
        _ => return None,
    })
}
