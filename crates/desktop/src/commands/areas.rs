use desktop_macros::klynt_command;
use desktop_shared::commands::{AreaCreateParams, AreaResponse, AreaUpdateParams};
use desktop_shared::CommandResult;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn area_list() -> Vec<AreaResponse> {
    state.area_list().await
}

#[klynt_command]
pub async fn area_create(app: tauri::AppHandle, params: AreaCreateParams) -> AreaResponse {
    let (result, updates) = state.area_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn area_update(app: tauri::AppHandle, params: AreaUpdateParams) -> AreaResponse {
    let (result, updates) = state.area_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn area_delete(app: tauri::AppHandle, id: String) -> bool {
    let (result, updates) = state.area_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn area_reorder(app: tauri::AppHandle, id: String, position: i32) -> AreaResponse {
    let (result, updates) = state.area_reorder(id, position).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
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
