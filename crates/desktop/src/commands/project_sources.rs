use desktop_macros::klynt_command;
use desktop_shared::entity_link_types::*;
#[klynt_command]
pub async fn project_source_create(params: ProjectSourceCreateParams) -> ProjectSourceResponse {
    let (result, updates) = state.project_source_create(params).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn project_source_delete(id: String) -> bool {
    let (result, updates) = state.project_source_delete(id).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn project_source_list(project_id: String) -> Vec<ProjectSourceResponse> {
    state.project_source_list(project_id).await
}
