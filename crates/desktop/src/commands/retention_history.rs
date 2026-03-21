use desktop_shared::commands::{RetentionHistoryParams, RetentionHistoryResponse};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn retention_history(
    state: State<'_, Arc<AppCore>>,
    params: RetentionHistoryParams,
) -> Result<RetentionHistoryResponse, ApiError> {
    state.retention_history(params).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &["retention_history"];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "retention_history" => {
            dev::val(core.retention_history(try_field!(dev::parse_params(body))).await)
        }
        _ => return None,
    })
}
