//! App-core handlers for coding-memory Phase-4 surfaces.

use coding_memory::recall::CodingRecallService;
use std::sync::Arc;

/// `OpHandler` for recall context requests from the `klyntbot-hook context` subcommand.
#[derive(Clone)]
pub struct RecallOpHandler {
    svc: Arc<CodingRecallService>,
}

impl RecallOpHandler {
    /// Construct with the shared recall service.
    #[must_use]
    #[tracing::instrument(skip(svc))]
    pub fn new(svc: Arc<CodingRecallService>) -> Self {
        Self { svc }
    }
}

#[async_trait::async_trait]
impl coding_ingest::daemon::OpHandler for RecallOpHandler {
    async fn handle(&self, payload: serde_json::Value) -> common::Result<serde_json::Value> {
        let op = payload
            .get("op")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let repo = payload.get("repo").and_then(|v| v.as_str());

        match op {
            "render_session_start" => {
                let md =
                    coding_memory::recall::renderers::render_session_start_block(&self.svc, repo)
                        .await?;
                Ok(serde_json::json!({"markdown": md}))
            }
            "render_user_prompt" => {
                let query = payload.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let md = coding_memory::recall::renderers::render_user_prompt_block(
                    &self.svc, query, repo,
                )
                .await?;
                Ok(serde_json::json!({"markdown": md}))
            }
            _ => Ok(serde_json::json!({"error": format!("unknown op: {op}")})),
        }
    }
}

// Handler functions (Tauri / dev-server adapters call these).

/// `coding_memory_recall_index` — wraps the toolset.
pub async fn recall_index_handler(
    svc: &Arc<CodingRecallService>,
    args: serde_json::Value,
) -> common::Result<serde_json::Value> {
    let toolset = coding_memory::CodingMemoryToolset::new(svc.clone());
    toolset.dispatch("recall_index", args).await
}

/// `coding_memory_recall_timeline`.
pub async fn recall_timeline_handler(
    svc: &Arc<CodingRecallService>,
    args: serde_json::Value,
) -> common::Result<serde_json::Value> {
    coding_memory::CodingMemoryToolset::new(svc.clone())
        .dispatch("recall_timeline", args)
        .await
}

/// `coding_memory_recall_fetch`.
pub async fn recall_fetch_handler(
    svc: &Arc<CodingRecallService>,
    args: serde_json::Value,
) -> common::Result<serde_json::Value> {
    coding_memory::CodingMemoryToolset::new(svc.clone())
        .dispatch("recall_fetch", args)
        .await
}

/// `coding_memory_check_dead_ends`.
pub async fn check_dead_ends_handler(
    svc: &Arc<CodingRecallService>,
    args: serde_json::Value,
) -> common::Result<serde_json::Value> {
    coding_memory::CodingMemoryToolset::new(svc.clone())
        .dispatch("check_dead_ends", args)
        .await
}

/// `coding_memory_recall_facts_as_of`.
pub async fn recall_facts_as_of_handler(
    svc: &Arc<CodingRecallService>,
    args: serde_json::Value,
) -> common::Result<serde_json::Value> {
    coding_memory::CodingMemoryToolset::new(svc.clone())
        .dispatch("recall_facts_as_of", args)
        .await
}

/// `coding_memory_recall_change_history`.
pub async fn recall_change_history_handler(
    svc: &Arc<CodingRecallService>,
    args: serde_json::Value,
) -> common::Result<serde_json::Value> {
    coding_memory::CodingMemoryToolset::new(svc.clone())
        .dispatch("recall_change_history", args)
        .await
}

/// `coding_memory_recall_decision_points`.
pub async fn recall_decision_points_handler(
    svc: &Arc<CodingRecallService>,
    args: serde_json::Value,
) -> common::Result<serde_json::Value> {
    coding_memory::CodingMemoryToolset::new(svc.clone())
        .dispatch("recall_decision_points", args)
        .await
}

/// `coding_memory_recall_log` — paginated list of telemetry rows.
pub async fn recall_log_handler(
    svc: &Arc<CodingRecallService>,
    layer: Option<String>,
    limit: i64,
    offset: i64,
) -> common::Result<Vec<coding_memory::RecallInvocationRow>> {
    svc.telemetry_repo()
        .list_recent(limit, offset, layer.as_deref())
        .await
}

/// `coding_memory_session_replay_recall_overlay` — by session id.
pub async fn session_recall_overlay_handler(
    svc: &Arc<CodingRecallService>,
    session_id: String,
    limit: i64,
    offset: i64,
) -> common::Result<Vec<coding_memory::RecallInvocationRow>> {
    svc.telemetry_repo()
        .list_by_session(&session_id, limit, offset)
        .await
}
