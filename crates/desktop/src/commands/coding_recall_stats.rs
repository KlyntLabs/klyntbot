use app_core::coding::recall_stats_handler::RecallStats;
use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;

#[klynt_command]
pub async fn coding_recall_stats(workspace_id: String, days: Option<u32>) -> RecallStats {
    state
        .coding_recall_stats(&workspace_id, days)
        .await
        .map_err(|e| ApiError::new("RECALL_STATS_ERROR", e.to_string()))
}
