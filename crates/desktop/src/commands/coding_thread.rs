use coding_agents_md::AgentsMdSource;
use desktop_macros::klynt_command;
use desktop_shared::coding::{ApprovalPolicy, Thread, ThreadSummary};
use desktop_shared::errors::ApiError;

use crate::commands::dev_helpers as dev;

#[klynt_command]
pub async fn coding_thread_start(
    workspace_id: String,
    model: Option<String>,
    approval_policy: Option<ApprovalPolicy>,
    ephemeral: Option<bool>,
) -> Thread {
    state
        .coding_thread_start(
            &workspace_id,
            model,
            approval_policy,
            ephemeral.unwrap_or(false),
        )
        .await
        .map_err(ApiError::from)
}

#[klynt_command]
pub async fn coding_thread_resume(thread_id: String, include_items: Option<bool>) -> Thread {
    state
        .coding_thread_resume(&thread_id, include_items.unwrap_or(true))
        .await
        .map_err(ApiError::from)
}

#[klynt_command]
pub async fn coding_thread_read(
    thread_id: String,
    cursor: Option<String>,
    limit: Option<i64>,
) -> Thread {
    state
        .coding_thread_read(&thread_id, cursor, limit)
        .await
        .map_err(ApiError::from)
}

#[klynt_command]
pub async fn coding_thread_list(
    workspace_id: Option<String>,
    cursor: Option<String>,
    limit: Option<i64>,
    sort_key: Option<String>,
) -> Vec<ThreadSummary> {
    state
        .coding_thread_list(workspace_id.as_deref(), cursor, limit, sort_key)
        .await
        .map_err(ApiError::from)
}

#[klynt_command]
pub async fn coding_thread_fork(thread_id: String, from_message_id: Option<String>) -> Thread {
    state
        .coding_thread_fork(&thread_id, from_message_id)
        .await
        .map_err(ApiError::from)
}

#[klynt_command]
pub async fn coding_thread_compact(thread_id: String) -> serde_json::Value {
    state
        .coding_thread_compact(&thread_id)
        .await
        .map_err(ApiError::from)
}

#[klynt_command]
pub async fn coding_thread_archive(thread_id: String) -> () {
    state
        .coding_thread_archive(&thread_id)
        .await
        .map_err(ApiError::from)
}

#[klynt_command]
pub async fn coding_thread_set_name(thread_id: String, name: String) -> () {
    state
        .coding_thread_set_name(&thread_id, &name)
        .await
        .map_err(ApiError::from)
}

#[klynt_command]
pub async fn coding_thread_subscribe(thread_id: String) -> serde_json::Value {
    let sub_id = state.coding_thread_subscribe(&thread_id).await?;
    Ok(serde_json::json!({ "subscriptionId": sub_id }))
}

#[klynt_command]
pub async fn coding_thread_unsubscribe(subscription_id: String) -> () {
    state
        .coding_thread_unsubscribe(&subscription_id)
        .await
        .map_err(ApiError::from)
}

#[klynt_command]
pub async fn coding_thread_refresh_agents_md(thread_id: String) -> serde_json::Value {
    state
        .coding_thread_refresh_agents_md(&thread_id)
        .await
        .map(|sources| serde_json::to_value(&sources).unwrap_or_default())
        .map_err(ApiError::from)
}

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &::app_core::state::AppCore,
    body: &serde_json::Value,
) -> Option<desktop_shared::CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "coding_thread_start" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            let model = body.get("model").and_then(|v| v.as_str()).map(String::from);
            let approval_policy: Option<ApprovalPolicy> = body
                .get("approvalPolicy")
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            let ephemeral = body
                .get("ephemeral")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            dev::val(
                core.coding_thread_start(&workspace_id, model, approval_policy, ephemeral)
                    .await
                    .map_err(ApiError::from),
            )
        }
        "coding_thread_resume" => {
            let thread_id = try_field!(dev::get_str(body, "threadId"));
            let include_items = body
                .get("includeItems")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            dev::val(
                core.coding_thread_resume(&thread_id, include_items)
                    .await
                    .map_err(ApiError::from),
            )
        }
        "coding_thread_read" => {
            let thread_id = try_field!(dev::get_str(body, "threadId"));
            let cursor = body
                .get("cursor")
                .and_then(|v| v.as_str())
                .map(String::from);
            let limit = body.get("limit").and_then(|v| v.as_i64());
            dev::val(
                core.coding_thread_read(&thread_id, cursor, limit)
                    .await
                    .map_err(ApiError::from),
            )
        }
        "coding_thread_list" => {
            let workspace_id = body.get("workspaceId").and_then(|v| v.as_str()).map(String::from);
            let cursor = body
                .get("cursor")
                .and_then(|v| v.as_str())
                .map(String::from);
            let limit = body.get("limit").and_then(|v| v.as_i64());
            let sort_key = body
                .get("sortKey")
                .and_then(|v| v.as_str())
                .map(String::from);
            dev::val(
                core.coding_thread_list(workspace_id.as_deref(), cursor, limit, sort_key)
                    .await
                    .map_err(ApiError::from),
            )
        }
        "coding_thread_fork" => {
            let thread_id = try_field!(dev::get_str(body, "threadId"));
            let from_message_id = body
                .get("fromMessageId")
                .and_then(|v| v.as_str())
                .map(String::from);
            dev::val(
                core.coding_thread_fork(&thread_id, from_message_id)
                    .await
                    .map_err(ApiError::from),
            )
        }
        "coding_thread_compact" => {
            let thread_id = try_field!(dev::get_str(body, "threadId"));
            dev::val(
                core.coding_thread_compact(&thread_id)
                    .await
                    .map_err(ApiError::from),
            )
        }
        "coding_thread_archive" => {
            let thread_id = try_field!(dev::get_str(body, "threadId"));
            dev::val(
                core.coding_thread_archive(&thread_id)
                    .await
                    .map_err(ApiError::from),
            )
        }
        "coding_thread_set_name" => {
            let thread_id = try_field!(dev::get_str(body, "threadId"));
            let name = try_field!(dev::get_str(body, "name"));
            dev::val(
                core.coding_thread_set_name(&thread_id, &name)
                    .await
                    .map_err(ApiError::from),
            )
        }
        "coding_thread_subscribe" => {
            let thread_id = try_field!(dev::get_str(body, "threadId"));
            match core.coding_thread_subscribe(&thread_id).await {
                Ok(sub_id) => Ok(serde_json::json!({ "subscriptionId": sub_id })),
                Err(e) => Err(ApiError::new("THREAD_ERROR", e.to_string())),
            }
        }
        "coding_thread_unsubscribe" => {
            let subscription_id = try_field!(dev::get_str(body, "subscriptionId"));
            dev::val(
                core.coding_thread_unsubscribe(&subscription_id)
                    .await
                    .map_err(ApiError::from),
            )
        }
        _ => return None,
    })
}
