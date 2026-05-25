use desktop_macros::klynt_command;
use desktop_shared::commands::{
    ProjectCreateParams, ProjectHealthMetricsResponse, ProjectResponse, ProjectUpdateParams,
};
#[klynt_command]
pub async fn project_create(params: ProjectCreateParams) -> ProjectResponse {
    let (result, updates) = state.project_create(params).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn project_get(id: String) -> ProjectResponse {
    state.project_get(id).await
}

#[klynt_command]
pub async fn project_update(params: ProjectUpdateParams) -> ProjectResponse {
    let (result, updates) = state.project_update(params).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn project_delete(id: String) -> bool {
    let (result, updates) = state.project_delete(id).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn project_archive(id: String) -> ProjectResponse {
    let (result, updates) = state.project_archive(id).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn project_update_instructions(
    id: String,
    instructions: desktop_shared::specta_helpers::JsonValueWrapper,
) -> ProjectResponse {
    let (result, updates) = state
        .project_update_instructions(id, instructions.0)
        .await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn project_update_role(id: String, role: String) -> ProjectResponse {
    let (result, updates) = state.project_update_role(id, role).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn project_health_metrics(project_id: String) -> ProjectHealthMetricsResponse {
    state.project_health_metrics(project_id).await
}
