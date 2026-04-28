use desktop_macros::klynt_command;
use desktop_shared::commands::{RetentionHistoryParams, RetentionHistoryResponse};
use desktop_shared::CommandResult;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn retention_history(days: i64, by_domain: Option<bool>) -> RetentionHistoryResponse {
    state
        .retention_history(RetentionHistoryParams {
            days,
            by_domain: by_domain.unwrap_or(false),
        })
        .await
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
        "retention_history" => dev::val(
            core.retention_history(try_field!(dev::parse_params(body)))
                .await,
        ),
        _ => return None,
    })
}
