use desktop_macros::klynt_command;
use desktop_shared::commands::ReviewStatsSummaryResponse;
#[klynt_command]
pub async fn review_stats_summary() -> ReviewStatsSummaryResponse {
    state.review_stats_summary().await
}
