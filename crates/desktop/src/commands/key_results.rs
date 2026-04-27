use desktop_macros::klynt_command;
use desktop_shared::commands::{KeyResultCreateParams, KeyResultResponse, KeyResultUpdateParams};
use desktop_shared::CommandResult;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn key_result_create(
    app: tauri::AppHandle,
    params: KeyResultCreateParams,
) -> KeyResultResponse {
    let (result, updates) = state.key_result_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn key_result_update(
    app: tauri::AppHandle,
    params: KeyResultUpdateParams,
) -> KeyResultResponse {
    let (result, updates) = state.key_result_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn key_result_update_metric(
    app: tauri::AppHandle,
    id: String,
    current_value: f64,
) -> KeyResultResponse {
    let (result, updates) = state.key_result_update_metric(id, current_value).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn key_result_delete(
    app: tauri::AppHandle,
    id: String,
) -> bool {
    let (result, updates) = state.key_result_delete(id).await?;
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
        "key_result_create" => dev::val_rh(
            core.key_result_create(try_field!(dev::parse_params(body)))
                .await,
        ),
        "key_result_update" => dev::val_rh(
            core.key_result_update(try_field!(dev::parse_params(body)))
                .await,
        ),
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
