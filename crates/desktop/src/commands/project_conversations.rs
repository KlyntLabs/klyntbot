use desktop_macros::klynt_command;
use desktop_shared::entity_link_types::SessionSummaryResponse;
#[klynt_command]
pub async fn project_conversations_list(project_id: String) -> Vec<SessionSummaryResponse> {
    state.project_conversations_list(project_id).await
}
