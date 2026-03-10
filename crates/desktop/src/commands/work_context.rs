use desktop_shared::commands::{
    ContextResumeResponse, ContextTimelineBlockResponse, WorkContextDetailResponse,
    WorkContextResponse, WorkContextUpdateParams,
};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn list_work_contexts(
    state: State<'_, Arc<AppCore>>,
    status: Option<String>,
) -> Result<Vec<WorkContextResponse>, ApiError> {
    state.list_work_contexts(status).await
}

#[tauri::command]
pub async fn get_work_context(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<Option<WorkContextResponse>, ApiError> {
    state.get_work_context(id).await
}

#[tauri::command]
pub async fn get_work_context_detail(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<WorkContextDetailResponse, ApiError> {
    state.get_work_context_detail(id).await
}

#[tauri::command]
pub async fn update_work_context(
    state: State<'_, Arc<AppCore>>,
    params: WorkContextUpdateParams,
) -> Result<WorkContextResponse, ApiError> {
    state.update_work_context(params).await
}

#[tauri::command]
pub async fn archive_work_context(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<WorkContextResponse, ApiError> {
    state.archive_work_context(id).await
}

#[tauri::command]
pub async fn merge_work_contexts(
    state: State<'_, Arc<AppCore>>,
    keep_id: String,
    remove_id: String,
) -> Result<WorkContextResponse, ApiError> {
    state.merge_work_contexts(keep_id, remove_id).await
}

#[tauri::command]
pub async fn search_work_contexts(
    state: State<'_, Arc<AppCore>>,
    query: String,
) -> Result<Vec<WorkContextResponse>, ApiError> {
    state.search_work_contexts(query).await
}

#[tauri::command]
pub async fn get_context_timeline(
    state: State<'_, Arc<AppCore>>,
    date: String,
    tz_offset_mins: Option<i32>,
) -> Result<Vec<ContextTimelineBlockResponse>, ApiError> {
    state.get_context_timeline(date, tz_offset_mins).await
}

#[tauri::command]
pub async fn get_context_resume_data(
    state: State<'_, Arc<AppCore>>,
    context_id: String,
) -> Result<ContextResumeResponse, ApiError> {
    state.get_context_resume_data(context_id).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "list_work_contexts",
    "get_work_context",
    "get_work_context_detail",
    "update_work_context",
    "archive_work_context",
    "merge_work_contexts",
    "search_work_contexts",
    "get_context_timeline",
    "get_context_resume_data",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "list_work_contexts" => dev::val(core.list_work_contexts(dev::get(body, "status")).await),
        "get_work_context" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.get_work_context(id).await)
        }
        "get_work_context_detail" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.get_work_context_detail(id).await)
        }
        "update_work_context" => dev::val(
            core.update_work_context(try_field!(dev::parse_params(body)))
                .await,
        ),
        "archive_work_context" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.archive_work_context(id).await)
        }
        "merge_work_contexts" => {
            let keep_id = try_field!(dev::get_str(body, "keepId"));
            let remove_id = try_field!(dev::get_str(body, "removeId"));
            dev::val(core.merge_work_contexts(keep_id, remove_id).await)
        }
        "search_work_contexts" => {
            let query = try_field!(dev::get_str(body, "query"));
            dev::val(core.search_work_contexts(query).await)
        }
        "get_context_timeline" => {
            let date = try_field!(dev::get_str(body, "date"));
            dev::val(
                core.get_context_timeline(date, dev::get(body, "tzOffsetMins"))
                    .await,
            )
        }
        "get_context_resume_data" => {
            let context_id = try_field!(dev::get_str(body, "contextId"));
            dev::val(core.get_context_resume_data(context_id).await)
        }
        _ => return None,
    })
}
