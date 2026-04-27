use desktop_shared::commands::{KnowledgeHealthSummary, TopicDetailParams, TopicDetailResponse};
use desktop_shared::CommandResult;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
#[specta::specta]
pub async fn knowledge_health_summary(
    state: State<'_, Arc<AppCore>>,
) -> CommandResult<KnowledgeHealthSummary> {
    state.knowledge_health_summary().await
}

#[tauri::command]
#[specta::specta]
pub async fn knowledge_topic_detail(
    state: State<'_, Arc<AppCore>>,
    params: TopicDetailParams,
) -> CommandResult<TopicDetailResponse> {
    state.knowledge_topic_detail(params).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &["knowledge_health_summary", "knowledge_topic_detail"];

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
