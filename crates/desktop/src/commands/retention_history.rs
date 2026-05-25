use desktop_macros::klynt_command;
use desktop_shared::commands::{RetentionHistoryParams, RetentionHistoryResponse};
#[klynt_command]
pub async fn retention_history(days: i64, by_domain: Option<bool>) -> RetentionHistoryResponse {
    state
        .retention_history(RetentionHistoryParams {
            days,
            by_domain: by_domain.unwrap_or(false),
        })
        .await
}
