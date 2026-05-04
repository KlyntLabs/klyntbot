use crate::AppCoreState;
use app_core::coding::recall_stats_handler::RecallStats;
use desktop_macros::klynt_command;
use std::sync::Arc;

#[klynt_command]
pub async fn coding_recall_stats(workspace_id: String, days: Option<u32>) -> RecallStats {
    core.coding_recall_stats(&workspace_id, days).await
}
