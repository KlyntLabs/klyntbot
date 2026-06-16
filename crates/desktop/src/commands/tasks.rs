use desktop_macros::klynt_command;
use desktop_shared::commands::{
    ObjectiveResponse, ProjectResponse, TaskCreateParams, TaskResponse, TaskUpdateParams,
    TodayTaskResponse,
};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use storage::{TaskAttachmentRow, TaskTimeEntryRow};

#[klynt_command]
pub async fn task_get(id: String) -> Option<TaskResponse> {
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
pub async fn task_create(params: TaskCreateParams) -> TaskResponse {
    let (result, updates) = state.task_create(params).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn task_update(params: TaskUpdateParams) -> TaskResponse {
    let (result, updates) = state.task_update(params, Some("user".into())).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn task_delete(id: String) -> bool {
    let (result, updates) = state.task_delete(id).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn task_toggle_complete(id: String) -> TaskResponse {
    let (result, updates) = state.task_toggle_complete(id).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn task_list_children(parent_id: String) -> Vec<TaskResponse> {
    state.task_list_children(parent_id).await
}

#[klynt_command]
pub async fn today_tasks() -> Vec<TodayTaskResponse> {
    state.today_tasks().await
}

#[klynt_command]
pub async fn project_list(area_id: Option<String>) -> Vec<ProjectResponse> {
    state.project_list_for_tasks(area_id).await
}

#[klynt_command]
pub async fn objective_list(project_id: Option<String>) -> Vec<ObjectiveResponse> {
    state.objective_list_for_tasks(project_id).await
}

// ── Dependencies ────────────────────────────────────────────────────

#[klynt_command]
pub async fn task_add_dependency(task_id: String, blocker_id: String) -> () {
    state.task_add_dependency(task_id, blocker_id).await
}

#[klynt_command]
pub async fn task_list_dependencies(task_id: String) -> Vec<TaskResponse> {
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
pub async fn task_list_attachments(task_id: String) -> Vec<TaskAttachmentRow> {
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
pub async fn task_list_time_entries(task_id: String) -> Vec<TaskTimeEntryRow> {
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
    let core = Arc::clone(state);
    tokio::spawn(async move {
        core.check_cross_domain_str(&domain, &id, &title, created_at.as_deref())
            .await;
    });
    Ok(())
}
