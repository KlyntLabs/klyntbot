//! Tauri commands for coding todo list.

use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;

/// Get the current agent's todo list for a thread.
#[klynt_command]
pub async fn coding_todo_get(thread_id: String) -> Vec<serde_json::Value> {
    state
        .coding_todo_get(&thread_id)
        .await
        .map_err(|e| ApiError::new("CODING_TODO_ERROR", e.to_string()))
}

/// Ratify a proposed plan.
#[klynt_command]
pub async fn coding_plan_ratify(thread_id: String, plan_session_id: String) -> bool {
    state
        .coding_plan_ratify(&thread_id, &plan_session_id)
        .await
        .map_err(|e| ApiError::new("CODING_PLAN_ERROR", e.to_string()))
}

/// User edited items in a proposed plan.
#[klynt_command]
pub async fn coding_plan_user_edit(
    thread_id: String,
    plan_session_id: String,
    items_json: String,
) -> bool {
    state
        .coding_plan_user_edit(&thread_id, &plan_session_id, &items_json)
        .await
        .map_err(|e| ApiError::new("CODING_PLAN_ERROR", e.to_string()))
}

/// User removed items from a proposed plan.
#[klynt_command]
pub async fn coding_plan_user_remove(
    thread_id: String,
    plan_session_id: String,
    item_ids: Vec<String>,
) -> bool {
    state
        .coding_plan_user_remove(&thread_id, &plan_session_id, &item_ids)
        .await
        .map_err(|e| ApiError::new("CODING_PLAN_ERROR", e.to_string()))
}
