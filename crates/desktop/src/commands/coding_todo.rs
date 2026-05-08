//! Tauri commands for coding todo list.

use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;

/// Get the current agent's todo list for a thread.
#[klynt_command]
pub async fn coding_todo_get(thread_id: String) -> serde_json::Value {
    state
        .coding_todo_get(&thread_id)
        .await
        .map_err(|e| ApiError::new("CODING_TODO_ERROR", e.to_string()))
        .and_then(|view| serde_json::to_value(view).map_err(|e| ApiError::new("SERIALIZE_ERROR", e.to_string())))
}

/// Ratify a proposed plan.
#[klynt_command]
pub async fn coding_plan_ratify(thread_id: String, plan_session_id: String) -> serde_json::Value {
    state
        .coding_plan_ratify(&thread_id, &plan_session_id)
        .await
        .map_err(|e| ApiError::new("CODING_PLAN_ERROR", e.to_string()))
        .and_then(|view| serde_json::to_value(view).map_err(|e| ApiError::new("SERIALIZE_ERROR", e.to_string())))
}

/// User edited items in a proposed plan.
#[klynt_command]
pub async fn coding_plan_user_edit(
    thread_id: String,
    plan_session_id: String,
    items_json: String,
) -> serde_json::Value {
    state
        .coding_plan_user_edit(&thread_id, &plan_session_id, &items_json)
        .await
        .map_err(|e| ApiError::new("CODING_PLAN_ERROR", e.to_string()))
        .and_then(|view| serde_json::to_value(view).map_err(|e| ApiError::new("SERIALIZE_ERROR", e.to_string())))
}

/// User removed items from a proposed plan.
#[klynt_command]
pub async fn coding_plan_user_remove(
    thread_id: String,
    plan_session_id: String,
    item_ids: Vec<String>,
) -> serde_json::Value {
    state
        .coding_plan_user_remove(&thread_id, &plan_session_id, &item_ids)
        .await
        .map_err(|e| ApiError::new("CODING_PLAN_ERROR", e.to_string()))
        .and_then(|view| serde_json::to_value(view).map_err(|e| ApiError::new("SERIALIZE_ERROR", e.to_string())))
}

/// Enter plan mode for a thread.
#[klynt_command]
pub async fn coding_plan_enter(thread_id: String) -> serde_json::Value {
    state
        .coding_plan_enter(&thread_id)
        .await
        .map_err(|e| ApiError::new("CODING_PLAN_ENTER_ERROR", e.to_string()))
        .and_then(|view| serde_json::to_value(view).map_err(|e| ApiError::new("SERIALIZE_ERROR", e.to_string())))
}

/// Cancel plan mode for a thread.
#[klynt_command]
pub async fn coding_plan_cancel(thread_id: String) -> serde_json::Value {
    state
        .coding_plan_cancel(&thread_id)
        .await
        .map_err(|e| ApiError::new("CODING_PLAN_CANCEL_ERROR", e.to_string()))
        .and_then(|view| serde_json::to_value(view).map_err(|e| ApiError::new("SERIALIZE_ERROR", e.to_string())))
}
