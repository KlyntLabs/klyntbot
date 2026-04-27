use app_core::AppCore;
use desktop_shared::types::{
    CronJobCreateParams, CronJobResponse, CronJobUpdateParams, CronStatusResponse,
};
use desktop_shared::CommandResult;
use desktop_macros::klynt_command;

#[klynt_command]
pub async fn cron_list(
    include_disabled: Option<bool>,
) -> Vec<CronJobResponse> {
    state.cron_list(include_disabled.unwrap_or(true)).await
}

#[klynt_command]
pub async fn cron_status() -> CronStatusResponse {
    state.cron_status().await
}

#[klynt_command]
pub async fn cron_enable(
    app: tauri::AppHandle,
    id: String,
    enabled: bool,
) -> CronJobResponse {
    let (result, updates) = state.cron_enable(id, enabled).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn cron_run(id: String) -> bool {
    state.cron_run(id).await
}

#[klynt_command]
pub async fn cron_delete(id: String) -> bool {
    state.cron_delete(id).await
}

#[klynt_command]
pub async fn cron_create(
    app: tauri::AppHandle,
    params: CronJobCreateParams,
) -> CronJobResponse {
    let (result, updates) = state.cron_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn cron_update(
    app: tauri::AppHandle,
    params: CronJobUpdateParams,
) -> CronJobResponse {
    let (result, updates) = state.cron_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

// ── Dev server dispatch ──

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
