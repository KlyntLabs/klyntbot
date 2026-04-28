use desktop_macros::klynt_command;
use desktop_shared::commands::{ObjectiveCreateParams, ObjectiveResponse, ObjectiveUpdateParams};
use desktop_shared::CommandResult;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn objective_create(
    app: tauri::AppHandle,
    params: ObjectiveCreateParams,
) -> ObjectiveResponse {
    let (result, updates) = state.objective_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn objective_get(id: String) -> ObjectiveResponse {
    state.objective_get(id).await
}

#[klynt_command]
pub async fn objective_update(
    app: tauri::AppHandle,
    params: ObjectiveUpdateParams,
) -> ObjectiveResponse {
    let (result, updates) = state.objective_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn objective_delete(app: tauri::AppHandle, id: String) -> bool {
    let (result, updates) = state.objective_delete(id).await?;
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
