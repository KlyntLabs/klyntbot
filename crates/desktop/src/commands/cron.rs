use std::sync::Arc;

use app_core::AppCore;
use desktop_shared::CommandResult;
use desktop_shared::types::{
    CronJobCreateParams, CronJobResponse, CronJobUpdateParams, CronStatusResponse,
};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn cron_list(
    state: State<'_, Arc<AppCore>>,
    include_disabled: Option<bool>,
) -> CommandResult<Vec<CronJobResponse>> {
    state.cron_list(include_disabled.unwrap_or(true)).await
}

#[tauri::command]
#[specta::specta]
pub async fn cron_status(state: State<'_, Arc<AppCore>>) -> CommandResult<CronStatusResponse> {
    state.cron_status().await
}

#[tauri::command]
#[specta::specta]
pub async fn cron_enable(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
    enabled: bool,
) -> CommandResult<CronJobResponse> {
    let (result, updates) = state.cron_enable(id, enabled).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn cron_run(state: State<'_, Arc<AppCore>>, id: String) -> CommandResult<bool> {
    state.cron_run(id).await
}

#[tauri::command]
#[specta::specta]
pub async fn cron_delete(state: State<'_, Arc<AppCore>>, id: String) -> CommandResult<bool> {
    state.cron_delete(id).await
}

#[tauri::command]
#[specta::specta]
pub async fn cron_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: CronJobCreateParams,
) -> CommandResult<CronJobResponse> {
    let (result, updates) = state.cron_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn cron_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: CronJobUpdateParams,
) -> CommandResult<CronJobResponse> {
    let (result, updates) = state.cron_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

// ── Dev server dispatch ──

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "cron_list",
    "cron_status",
    "cron_enable",
    "cron_run",
    "cron_delete",
    "cron_create",
    "cron_update",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "cron_list" => dev::val(
            core.cron_list(dev::get(body, "includeDisabled").unwrap_or(true))
                .await,
        ),
        "cron_status" => dev::val(core.cron_status().await),
        "cron_enable" => {
            let id = try_field!(dev::get_str(body, "id"));
            let enabled = try_field!(dev::require::<bool>(body, "enabled"));
            dev::val_rh(core.cron_enable(id, enabled).await)
        }
        "cron_run" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.cron_run(id).await)
        }
        "cron_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.cron_delete(id).await)
        }
        "cron_create" => dev::val_rh(core.cron_create(try_field!(dev::parse_params(body))).await),
        "cron_update" => dev::val_rh(core.cron_update(try_field!(dev::parse_params(body))).await),
        _ => return None,
    })
}
