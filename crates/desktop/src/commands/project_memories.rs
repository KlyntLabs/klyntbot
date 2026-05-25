use desktop_macros::klynt_command;
use desktop_shared::cognitive_commands::SemanticFactResponse;
#[klynt_command]
pub async fn project_memories_list(project_id: String) -> Vec<SemanticFactResponse> {
    state.project_memories_list(project_id).await
}

#[klynt_command]
pub async fn project_memories_by_type(
    project_id: String,
    memory_type: String,
) -> Vec<SemanticFactResponse> {
    state
        .project_memories_by_type(project_id, memory_type)
        .await
}
