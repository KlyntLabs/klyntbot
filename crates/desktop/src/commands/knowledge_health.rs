use desktop_shared::commands::{KnowledgeHealthSummary, TopicDetailParams, TopicDetailResponse};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn knowledge_health_summary(
    state: State<'_, Arc<AppCore>>,
) -> Result<KnowledgeHealthSummary, ApiError> {
    state.knowledge_health_summary().await
}

#[tauri::command]
pub async fn knowledge_topic_detail(
    state: State<'_, Arc<AppCore>>,
    params: TopicDetailParams,
) -> Result<TopicDetailResponse, ApiError> {
    state.knowledge_topic_detail(params).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] =
    &["knowledge_health_summary", "knowledge_topic_detail"];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "knowledge_health_summary" => dev::val(core.knowledge_health_summary().await),
        "knowledge_topic_detail" => {
            dev::val(core.knowledge_topic_detail(try_field!(dev::parse_params(body))).await)
        }
        _ => return None,
    })
}
