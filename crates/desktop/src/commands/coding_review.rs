use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;

use crate::commands::dev_helpers as dev;

#[klynt_command]
pub async fn coding_review_start(
    thread_id: String,
    target: Option<String>,
    delivery: Option<String>,
) -> serde_json::Value {
    state
        .coding_review_start(&thread_id, target.as_deref(), delivery.as_deref())
        .await
        .map(|r| serde_json::to_value(r).unwrap_or_default())
        .map_err(|e| ApiError::new("REVIEW_ERROR", e.to_string()))
}

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &::app_core::state::AppCore,
    body: &serde_json::Value,
) -> Option<desktop_shared::CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "coding_review_start" => {
            let thread_id = try_field!(dev::get_str(body, "threadId"));
            let target = body
                .get("target")
                .and_then(|v| v.as_str())
                .map(String::from);
            let delivery = body
                .get("delivery")
                .and_then(|v| v.as_str())
                .map(String::from);
            match core
                .coding_review_start(&thread_id, target.as_deref(), delivery.as_deref())
                .await
            {
                Ok(r) => Ok(serde_json::to_value(r).unwrap_or_default()),
                Err(e) => Err(ApiError::new("REVIEW_ERROR", e.to_string())),
            }
        }
        _ => return None,
    })
}
