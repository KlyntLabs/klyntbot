use desktop_macros::klynt_command;
use desktop_shared::commands::{AreaCreateParams, AreaResponse, AreaUpdateParams};

#[klynt_command]
pub async fn area_list() -> Vec<AreaResponse> {
    state.area_list().await
}

#[klynt_command]
pub async fn area_create(params: AreaCreateParams) -> AreaResponse {
    let (result, updates) = state.area_create(params).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn area_update(params: AreaUpdateParams) -> AreaResponse {
    let (result, updates) = state.area_update(params).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn area_delete(id: String) -> bool {
    let (result, updates) = state.area_delete(id).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn area_reorder(id: String, position: i32) -> AreaResponse {
    let (result, updates) = state.area_reorder(id, position).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}
