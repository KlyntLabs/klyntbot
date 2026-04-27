use desktop_shared::commands::{KnowledgeHealthSummary, TopicDetailParams, TopicDetailResponse};
use desktop_shared::CommandResult;
use desktop_macros::klynt_command;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn knowledge_health_summary() -> KnowledgeHealthSummary {
    state.knowledge_health_summary().await
}

#[klynt_command]
pub async fn knowledge_topic_detail(
    params: TopicDetailParams,
) -> TopicDetailResponse {
    state.knowledge_topic_detail(params).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "knowledge_health_summary" => dev::val(core.knowledge_health_summary().await),
        "knowledge_topic_detail" => dev::val(
            core.knowledge_topic_detail(try_field!(dev::parse_params(body)))
                .await,
        ),
        _ => return None,
    })
}
