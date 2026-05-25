use desktop_macros::klynt_command;
use desktop_shared::commands::{KnowledgeHealthSummary, TopicDetailParams, TopicDetailResponse};

#[klynt_command]
pub async fn knowledge_health_summary() -> KnowledgeHealthSummary {
    state.knowledge_health_summary().await
}

#[klynt_command]
pub async fn knowledge_topic_detail(params: TopicDetailParams) -> TopicDetailResponse {
    state.knowledge_topic_detail(params).await
}
