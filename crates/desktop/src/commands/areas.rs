use desktop_shared::commands::{AreaCreateParams, AreaResponse, AreaUpdateParams};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn area_list(state: State<'_, Arc<AppCore>>) -> Result<Vec<AreaResponse>, ApiError> {
    state.area_list().await
}

#[tauri::command]
pub async fn area_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: AreaCreateParams,
) -> Result<AreaResponse, ApiError> {
    let (result, updates) = state.area_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn area_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: AreaUpdateParams,
) -> Result<AreaResponse, ApiError> {
    let (result, updates) = state.area_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn area_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let (result, updates) = state.area_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn area_reorder(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
    position: i32,
) -> Result<AreaResponse, ApiError> {
    let (result, updates) = state.area_reorder(id, position).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "area_list",
    "area_create",
    "area_update",
    "area_delete",
    "area_reorder",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "area_list" => dev::val(core.area_list().await),
        "area_create" => dev::val_rh(core.area_create(try_field!(dev::parse_params(body))).await),
        "area_update" => dev::val_rh(core.area_update(try_field!(dev::parse_params(body))).await),
        "area_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val_rh(core.area_delete(id).await)
        }
        "area_reorder" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val_rh(
                core.area_reorder(id, dev::get(body, "position").unwrap_or(0))
                    .await,
            )
        }
        _ => return None,
    })
}
