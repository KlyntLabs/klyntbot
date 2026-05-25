use desktop_macros::klynt_command;
use desktop_shared::commands::{ObjectiveCreateParams, ObjectiveResponse, ObjectiveUpdateParams};
#[klynt_command]
pub async fn objective_create(params: ObjectiveCreateParams) -> ObjectiveResponse {
    let (result, updates) = state.objective_create(params).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn objective_get(id: String) -> ObjectiveResponse {
    state.objective_get(id).await
}

#[klynt_command]
pub async fn objective_update(params: ObjectiveUpdateParams) -> ObjectiveResponse {
    let (result, updates) = state.objective_update(params).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn objective_delete(id: String) -> bool {
    let (result, updates) = state.objective_delete(id).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}
