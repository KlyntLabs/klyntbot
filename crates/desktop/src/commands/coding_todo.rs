//! Tauri commands for coding todo list.

use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;
use feature_coding_todo::view::CodingTodoView;

/// Get the current agent's todo list for a thread.
#[klynt_command]
pub async fn coding_todo_get(thread_id: String) -> CodingTodoView {
    state
        .coding_todo_get(&thread_id)
        .await
        .map_err(ApiError::from)
}

/// Ratify a proposed plan.
#[klynt_command]
pub async fn coding_plan_ratify(thread_id: String, plan_session_id: String) -> CodingTodoView {
    state
        .coding_plan_ratify(&thread_id, &plan_session_id)
        .await
        .map_err(ApiError::from)
}

/// User edited items in a proposed plan.
#[klynt_command]
pub async fn coding_plan_user_edit(
    thread_id: String,
    plan_session_id: String,
    items_json: String,
) -> CodingTodoView {
    state
        .coding_plan_user_edit(&thread_id, &plan_session_id, &items_json)
        .await
        .map_err(ApiError::from)
}

/// User removed items from a proposed plan.
#[klynt_command]
pub async fn coding_plan_user_remove(
    thread_id: String,
    plan_session_id: String,
    item_ids: Vec<String>,
) -> CodingTodoView {
    state
        .coding_plan_user_remove(&thread_id, &plan_session_id, &item_ids)
        .await
        .map_err(ApiError::from)
}

/// Enter plan mode for a thread.
#[klynt_command]
pub async fn coding_plan_enter(thread_id: String) -> CodingTodoView {
    state
        .coding_plan_enter(&thread_id)
        .await
        .map_err(ApiError::from)
}

/// Cancel plan mode for a thread.
#[klynt_command]
pub async fn coding_plan_cancel(thread_id: String) -> CodingTodoView {
    state
        .coding_plan_cancel(&thread_id)
        .await
        .map_err(ApiError::from)
}

/// Open a plan file in the system default editor.
#[klynt_command]
pub async fn coding_plan_open_file(path: String) -> () {
    let _ = open::that(&path);
    Ok(())
}
