use desktop_shared::commands::{ObjectiveCreateParams, ObjectiveResponse, ObjectiveUpdateParams};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn objective_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: ObjectiveCreateParams,
) -> Result<ObjectiveResponse, ApiError> {
    let (result, updates) = state.objective_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn objective_get(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<ObjectiveResponse, ApiError> {
    state.objective_get(id).await
}

#[tauri::command]
pub async fn objective_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: ObjectiveUpdateParams,
) -> Result<ObjectiveResponse, ApiError> {
    let (result, updates) = state.objective_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn objective_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let (result, updates) = state.objective_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "objective_create",
    "objective_get",
    "objective_update",
    "objective_delete",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "objective_create" => dev::val_rh(
            core.objective_create(try_field!(dev::parse_params(body)))
                .await,
        ),
        "objective_get" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.objective_get(id).await)
        }
        "objective_update" => dev::val_rh(
            core.objective_update(try_field!(dev::parse_params(body)))
                .await,
        ),
        "objective_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val_rh(core.objective_delete(id).await)
        }
        _ => return None,
    })
}
