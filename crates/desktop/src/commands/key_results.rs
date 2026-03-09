use desktop_shared::commands::{KeyResultCreateParams, KeyResultResponse, KeyResultUpdateParams};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn key_result_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: KeyResultCreateParams,
) -> Result<KeyResultResponse, ApiError> {
    let (result, updates) = state.key_result_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn key_result_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: KeyResultUpdateParams,
) -> Result<KeyResultResponse, ApiError> {
    let (result, updates) = state.key_result_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn key_result_update_metric(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
    current_value: f64,
) -> Result<KeyResultResponse, ApiError> {
    let (result, updates) = state.key_result_update_metric(id, current_value).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn key_result_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let (result, updates) = state.key_result_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "key_result_create",
    "key_result_update",
    "key_result_update_metric",
    "key_result_delete",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "key_result_create" => {
            dev::val_rh(core.key_result_create(try_field!(dev::parse_params(body))).await)
        }
        "key_result_update" => {
            dev::val_rh(core.key_result_update(try_field!(dev::parse_params(body))).await)
        }
        "key_result_update_metric" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val_rh(
                core.key_result_update_metric(id, dev::get(body, "currentValue").unwrap_or(0.0))
                    .await,
            )
        }
        "key_result_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val_rh(core.key_result_delete(id).await)
        }
        _ => return None,
    })
}
