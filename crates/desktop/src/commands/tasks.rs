use desktop_shared::commands::{
    ObjectiveResponse, ProjectResponse, TaskCreateParams, TaskResponse, TaskUpdateParams,
    TodayTaskResponse,
};
use desktop_shared::{errors::ApiError, CommandResult};
use std::sync::Arc;
use storage::{TaskAttachmentRow, TaskTimeEntryRow};
use desktop_macros::klynt_command;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn task_get(
    id: String,
) -> Option<TaskResponse> {
    state.task_get(id).await
}

#[klynt_command]
pub async fn task_list(
    area_id: Option<String>,
    project_id: Option<String>,
    status: Option<String>,
) -> Vec<TaskResponse> {
    state.task_list(area_id, project_id, status).await
}

#[klynt_command]
pub async fn task_create(
    app: tauri::AppHandle,
    params: TaskCreateParams,
) -> TaskResponse {
    let (result, updates) = state.task_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn task_update(
    app: tauri::AppHandle,
    params: TaskUpdateParams,
) -> TaskResponse {
    let (result, updates) = state.task_update(params, Some("user".into())).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn task_delete(
    app: tauri::AppHandle,
    id: String,
) -> bool {
    let (result, updates) = state.task_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn task_toggle_complete(
    app: tauri::AppHandle,
    id: String,
) -> TaskResponse {
    let (result, updates) = state.task_toggle_complete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn task_list_children(
    parent_id: String,
) -> Vec<TaskResponse> {
    state.task_list_children(parent_id).await
}

#[klynt_command]
pub async fn today_tasks() -> Vec<TodayTaskResponse> {
    state.today_tasks().await
}

#[klynt_command]
pub async fn project_list(
    area_id: Option<String>,
) -> Vec<ProjectResponse> {
    state.project_list_for_tasks(area_id).await
}

#[klynt_command]
pub async fn objective_list(
    project_id: Option<String>,
) -> Vec<ObjectiveResponse> {
    state.objective_list_for_tasks(project_id).await
}

// ── Dependencies ────────────────────────────────────────────────────

#[klynt_command]
pub async fn task_add_dependency(
    task_id: String,
    blocker_id: String,
) -> () {
    state.task_add_dependency(task_id, blocker_id).await
}

#[klynt_command]
pub async fn task_list_dependencies(
    task_id: String,
) -> Vec<TaskResponse> {
    state.task_list_dependencies(task_id).await
}

// ── Attachments ─────────────────────────────────────────────────────

#[klynt_command]
pub async fn task_add_attachment(
    task_id: String,
    attachment_type: String,
    value: String,
    title: Option<String>,
) -> TaskAttachmentRow {
    state
        .task_add_attachment(task_id, attachment_type, value, title)
        .await
}

#[klynt_command]
pub async fn task_list_attachments(
    task_id: String,
) -> Vec<TaskAttachmentRow> {
    state.task_list_attachments(task_id).await
}

// ── Time entries ────────────────────────────────────────────────────

#[klynt_command]
pub async fn task_add_time_entry(
    task_id: String,
    started_at: String,
    duration_secs: Option<i64>,
    note: Option<String>,
) -> TaskTimeEntryRow {
    let started_at = started_at
        .parse::<jiff::Timestamp>()
        .map_err(|e| ApiError::new("VALIDATION", format!("invalid started_at: {e}")))?;
    state
        .task_add_time_entry(task_id, started_at, duration_secs, note)
        .await
}

#[klynt_command]
pub async fn task_list_time_entries(
    task_id: String,
) -> Vec<TaskTimeEntryRow> {
    state.task_list_time_entries(task_id).await
}

/// Fire-and-forget cross-domain check. Called by frontend when a detail view mounts.
#[klynt_command]
pub async fn cross_domain_check(
    domain: String,
    id: String,
    title: String,
    created_at: Option<String>,
) -> () {
    let core = Arc::clone(&*state);
    tokio::spawn(async move {
        core.check_cross_domain_str(&domain, &id, &title, created_at.as_deref())
            .await;
    });
    Ok(())
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
        "task_add_dependency" => {
            let task_id = try_field!(dev::get_str(body, "taskId"));
            let blocker_id = try_field!(dev::get_str(body, "blockerId"));
            dev::val(core.task_add_dependency(task_id, blocker_id).await)
        }
        "task_list_dependencies" => {
            let task_id = try_field!(dev::get_str(body, "taskId"));
            dev::val(core.task_list_dependencies(task_id).await)
        }
        "task_add_attachment" => {
            let task_id = try_field!(dev::get_str(body, "taskId"));
            let attachment_type = try_field!(dev::get_str(body, "attachmentType"));
            let value = try_field!(dev::get_str(body, "value"));
            let title: Option<String> = dev::get(body, "title");
            dev::val(
                core.task_add_attachment(task_id, attachment_type, value, title)
                    .await,
            )
        }
        "task_list_attachments" => {
            let task_id = try_field!(dev::get_str(body, "taskId"));
            dev::val(core.task_list_attachments(task_id).await)
        }
        "task_add_time_entry" => {
            let task_id = try_field!(dev::get_str(body, "taskId"));
            let started_at_str = try_field!(dev::get_str(body, "startedAt"));
            let started_at = started_at_str.parse::<jiff::Timestamp>().map_err(|e| {
                desktop_shared::errors::ApiError::new(
                    "VALIDATION",
                    format!("invalid startedAt: {e}"),
                )
            });
            let started_at = try_field!(started_at);
            let duration_secs: Option<i64> = dev::get(body, "durationSecs");
            let note: Option<String> = dev::get(body, "note");
            dev::val(
                core.task_add_time_entry(task_id, started_at, duration_secs, note)
                    .await,
            )
        }
        "task_list_time_entries" => {
            let task_id = try_field!(dev::get_str(body, "taskId"));
            dev::val(core.task_list_time_entries(task_id).await)
        }
        "cross_domain_check" => {
            let domain = try_field!(dev::get_str(body, "domain"));
            let id = try_field!(dev::get_str(body, "id"));
            let title = try_field!(dev::get_str(body, "title"));
            let created_at: Option<String> = dev::get(body, "createdAt");
            core.check_cross_domain_str(&domain, &id, &title, created_at.as_deref())
                .await;
            dev::val(Ok::<(), ApiError>(()))
        }
        _ => return None,
    })
}
