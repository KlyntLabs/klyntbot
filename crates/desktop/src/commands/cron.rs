use desktop_macros::klynt_command;
use desktop_shared::types::{
    CronJobCreateParams, CronJobResponse, CronJobUpdateParams, CronStatusResponse,
};

#[klynt_command]
pub async fn cron_list(include_disabled: Option<bool>) -> Vec<CronJobResponse> {
    state.cron_list(include_disabled.unwrap_or(true)).await
}

#[klynt_command]
pub async fn cron_status() -> CronStatusResponse {
    state.cron_status().await
}

#[klynt_command]
pub async fn cron_enable(id: String, enabled: bool) -> CronJobResponse {
    let (result, updates) = state.cron_enable(id, enabled).await?;
    super::emit_updates_ev(emitter, &updates);
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
pub async fn cron_create(params: CronJobCreateParams) -> CronJobResponse {
    let (result, updates) = state.cron_create(params).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn cron_update(params: CronJobUpdateParams) -> CronJobResponse {
    let (result, updates) = state.cron_update(params).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}
