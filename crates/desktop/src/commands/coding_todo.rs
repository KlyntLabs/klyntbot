//! Tauri commands for coding todo list.

use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;

fn ok_json<T: serde::Serialize>(result: Result<T, ApiError>) -> Result<serde_json::Value, ApiError> {
    result.and_then(|v| serde_json::to_value(v).map_err(|e| ApiError::new("SERIALIZE_ERROR", e.to_string())))
}

/// Get the current agent's todo list for a thread.
#[klynt_command]
pub async fn coding_todo_get(thread_id: String) -> serde_json::Value {
    ok_json(
        state
            .coding_todo_get(&thread_id)
            .await
            .map_err(|e| ApiError::new("CODING_TODO_ERROR", e.to_string())),
    )
    .unwrap_or_default()
}

/// Ratify a proposed plan.
#[klynt_command]
pub async fn coding_plan_ratify(thread_id: String, plan_session_id: String) -> serde_json::Value {
    ok_json(
        state
            .coding_plan_ratify(&thread_id, &plan_session_id)
            .await
            .map_err(|e| ApiError::new("CODING_PLAN_ERROR", e.to_string())),
    )
    .unwrap_or_default()
}

/// User edited items in a proposed plan.
#[klynt_command]
pub async fn coding_plan_user_edit(
    thread_id: String,
    plan_session_id: String,
    items_json: String,
) -> serde_json::Value {
    ok_json(
        state
            .coding_plan_user_edit(&thread_id, &plan_session_id, &items_json)
            .await
            .map_err(|e| ApiError::new("CODING_PLAN_ERROR", e.to_string())),
    )
    .unwrap_or_default()
}

/// User removed items from a proposed plan.
#[klynt_command]
pub async fn coding_plan_user_remove(
    thread_id: String,
    plan_session_id: String,
    item_ids: Vec<String>,
) -> serde_json::Value {
    ok_json(
        state
            .coding_plan_user_remove(&thread_id, &plan_session_id, &item_ids)
            .await
            .map_err(|e| ApiError::new("CODING_PLAN_ERROR", e.to_string())),
    )
    .unwrap_or_default()
}

/// Enter plan mode for a thread.
#[klynt_command]
pub async fn coding_plan_enter(thread_id: String) -> serde_json::Value {
    ok_json(
        state
            .coding_plan_enter(&thread_id)
            .await
            .map_err(|e| ApiError::new("CODING_PLAN_ENTER_ERROR", e.to_string())),
    )
    .unwrap_or_default()
}

/// Cancel plan mode for a thread.
#[klynt_command]
pub async fn coding_plan_cancel(thread_id: String) -> serde_json::Value {
    ok_json(
        state
            .coding_plan_cancel(&thread_id)
            .await
            .map_err(|e| ApiError::new("CODING_PLAN_CANCEL_ERROR", e.to_string())),
    )
    .unwrap_or_default()
}

/// Open a plan file in the system default editor.
#[klynt_command]
pub async fn coding_plan_open_file(path: String) {
    let _ = open::that(&path);
}
