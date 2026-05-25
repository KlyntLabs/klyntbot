use desktop_macros::klynt_command;
use desktop_shared::commands::{KeyResultCreateParams, KeyResultResponse, KeyResultUpdateParams};

#[klynt_command]
pub async fn key_result_create(params: KeyResultCreateParams) -> KeyResultResponse {
    let (result, updates) = state.key_result_create(params).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn key_result_update(params: KeyResultUpdateParams) -> KeyResultResponse {
    let (result, updates) = state.key_result_update(params).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn key_result_update_metric(id: String, current_value: f64) -> KeyResultResponse {
    let (result, updates) = state.key_result_update_metric(id, current_value).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn key_result_delete(id: String) -> bool {
    let (result, updates) = state.key_result_delete(id).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}
