use desktop_shared::commands::{
    DecompositionResponse, ObjectiveResponse, ProjectResponse,
    SuggestionResponse, TaskCreateParams, TaskResponse, TaskUpdateParams, TodayTaskResponse,
};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn task_start_focus(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    task_id: String,
) -> Result<TaskResponse, ApiError> {
    let (response, updates) = state.start_focus(task_id).await?;
    super::emit_updates(&app, &updates);
    Ok(response)
}

#[tauri::command]
pub async fn task_end_focus(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    task_id: String,
) -> Result<Option<TaskResponse>, ApiError> {
    match state.end_focus(task_id).await? {
        Some((response, updates)) => {
            super::emit_updates(&app, &updates);
            Ok(Some(response))
        }
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn task_get(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<Option<TaskResponse>, ApiError> {
    state.task_get(id).await
}

#[tauri::command]
pub async fn task_list(
    state: State<'_, Arc<AppCore>>,
    area_id: Option<String>,
    project_id: Option<String>,
    status: Option<String>,
) -> Result<Vec<TaskResponse>, ApiError> {
    state.task_list(area_id, project_id, status).await
}

#[tauri::command]
pub async fn task_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: TaskCreateParams,
) -> Result<TaskResponse, ApiError> {
    let (result, updates) = state.task_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn task_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: TaskUpdateParams,
) -> Result<TaskResponse, ApiError> {
    let (result, updates) = state.task_update(params, Some("user".into())).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn task_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let (result, updates) = state.task_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn task_toggle_complete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<TaskResponse, ApiError> {
    let (result, updates) = state.task_toggle_complete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn task_list_children(
    state: State<'_, Arc<AppCore>>,
    parent_id: String,
) -> Result<Vec<TaskResponse>, ApiError> {
    state.task_list_children(parent_id).await
}

#[tauri::command]
pub async fn today_tasks(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<TodayTaskResponse>, ApiError> {
    state.today_tasks().await
}

#[tauri::command]
pub async fn project_list(
    state: State<'_, Arc<AppCore>>,
    area_id: Option<String>,
) -> Result<Vec<ProjectResponse>, ApiError> {
    state.project_list_for_tasks(area_id).await
}

#[tauri::command]
pub async fn objective_list(
    state: State<'_, Arc<AppCore>>,
    project_id: Option<String>,
) -> Result<Vec<ObjectiveResponse>, ApiError> {
    state.objective_list_for_tasks(project_id).await
}

#[tauri::command]
pub async fn task_get_suggestions(
    state: State<'_, Arc<AppCore>>,
    task_id: String,
) -> Result<Vec<SuggestionResponse>, ApiError> {
    state.task_get_suggestions(task_id).await
}

#[tauri::command]
pub async fn task_apply_suggestion(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    suggestion_id: String,
) -> Result<TaskResponse, ApiError> {
    let (response, updates) = state.task_apply_suggestion(suggestion_id).await?;
    super::emit_updates(&app, &updates);
    Ok(response)
}

#[tauri::command]
pub async fn task_dismiss_suggestion(
    state: State<'_, Arc<AppCore>>,
    suggestion_id: String,
) -> Result<(), ApiError> {
    state.task_dismiss_suggestion(suggestion_id).await
}

#[tauri::command]
pub async fn task_decompose(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    task_id: String,
) -> Result<DecompositionResponse, ApiError> {
    let (response, updates) = state.task_decompose(task_id).await?;
    super::emit_updates(&app, &updates);
    Ok(response)
}

#[tauri::command]
pub async fn task_apply_decomposition(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    decomposition_id: String,
) -> Result<Vec<TaskResponse>, ApiError> {
    let (response, updates) = state.task_apply_decomposition(decomposition_id).await?;
    super::emit_updates(&app, &updates);
    Ok(response)
}

#[tauri::command]
pub async fn task_reject_decomposition(
    state: State<'_, Arc<AppCore>>,
    decomposition_id: String,
) -> Result<(), ApiError> {
    state.task_reject_decomposition(decomposition_id).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "task_list",
    "task_get",
    "task_create",
    "task_update",
    "task_delete",
    "task_toggle_complete",
    "task_list_children",
    "today_tasks",
    "project_list",
    "objective_list",
    "task_start_focus",
    "task_end_focus",
    "task_get_suggestions",
    "task_apply_suggestion",
    "task_dismiss_suggestion",
    "task_decompose",
    "task_apply_decomposition",
    "task_reject_decomposition",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "task_list" => dev::val(
            core.task_list(
                dev::get(body, "area_id"),
                dev::get(body, "project_id"),
                dev::get(body, "status"),
            )
            .await,
        ),
        "task_get" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.task_get(id).await)
        }
        "task_create" => dev::val_rh(core.task_create(try_field!(dev::parse_params(body))).await),
        "task_update" => dev::val_rh(
            core.task_update(try_field!(dev::parse_params(body)), Some("user".into()))
                .await,
        ),
        "task_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val_rh(core.task_delete(id).await)
        }
        "task_toggle_complete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val_rh(core.task_toggle_complete(id).await)
        }
        "task_list_children" => {
            let parent_id = try_field!(dev::get_str(body, "parentId"));
            dev::val(core.task_list_children(parent_id).await)
        }
        "today_tasks" => dev::val(core.today_tasks().await),
        "project_list" => dev::val(core.project_list_for_tasks(dev::get(body, "area_id")).await),
        "objective_list" => dev::val(
            core.objective_list_for_tasks(dev::get(body, "project_id"))
                .await,
        ),
        "task_start_focus" => {
            let task_id = try_field!(dev::get_str(body, "taskId"));
            dev::val_rh(core.start_focus(task_id).await)
        }
        "task_end_focus" => {
            let task_id = try_field!(dev::get_str(body, "taskId"));
            match core.end_focus(task_id).await {
                Ok(Some((resp, _))) => dev::val(Ok::<_, ApiError>(Some(resp))),
                Ok(None) => dev::val(Ok::<_, ApiError>(None::<TaskResponse>)),
                Err(e) => dev::val(Err::<Option<TaskResponse>, _>(e)),
            }
        }
        "task_get_suggestions" => {
            let task_id = try_field!(dev::get_str(body, "taskId"));
            dev::val(core.task_get_suggestions(task_id).await)
        }
        "task_apply_suggestion" => {
            let suggestion_id = try_field!(dev::get_str(body, "suggestionId"));
            dev::val_rh(core.task_apply_suggestion(suggestion_id).await)
        }
        "task_dismiss_suggestion" => {
            let suggestion_id = try_field!(dev::get_str(body, "suggestionId"));
            dev::val(core.task_dismiss_suggestion(suggestion_id).await)
        }
        "task_decompose" => dev::val_rh(
            core.task_decompose(try_field!(dev::get_str(body, "taskId")).into())
                .await,
        ),
        "task_apply_decomposition" => dev::val_rh(
            core.task_apply_decomposition(
                try_field!(dev::get_str(body, "decompositionId")).into(),
            )
            .await,
        ),
        "task_reject_decomposition" => dev::val(
            core.task_reject_decomposition(
                try_field!(dev::get_str(body, "decompositionId")).into(),
            )
            .await,
        ),
        _ => return None,
    })
}
