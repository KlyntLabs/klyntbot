use chrono::{DateTime, Utc};
use desktop_shared::commands::{
    KeyResultResponse, ObjectiveResponse, ProjectResponse, StatusLabelResponse, TaskCreateParams,
    TaskResponse, TaskUpdateParams, TodayTaskResponse,
};
use desktop_shared::errors::ApiError;
use desktop_shared::types::EntityKind;
use std::sync::Arc;
use storage::{ActionFilter, ActionPatch, ActionRow, KeyResultRow, ObjectiveRow, ProjectFilter};
use tauri::State;

use crate::app_core::AppCore;

// ── Row → Response converters ───────────────────────────────────────────

fn priority_label(p: Option<i16>) -> Option<String> {
    p.map(|v| format!("P{v}"))
}

pub(crate) fn action_to_task(
    row: &ActionRow,
    subtask_count: u32,
    subtask_completed_count: u32,
) -> TaskResponse {
    TaskResponse {
        id: row.id.clone(),
        title: row.title.clone(),
        completed: row.status == "done",
        priority: priority_label(row.priority),
        status: row.status.clone(),
        due_date: row.due_date.map(|d| d.format("%Y-%m-%d").to_string()),
        tags: row.tags.clone(),
        project_id: row.project_id.clone(),
        area_id: row.area_id.clone(),
        objective_id: row.key_result_id.clone(),
        description: row.description.clone(),
        parent_id: row.parent_id.clone(),
        subtask_count,
        subtask_completed_count,
        status_label_id: row.status_label_id.clone(),
        status_label: None,
    }
}

pub(crate) fn action_to_today_task(row: &ActionRow, now: DateTime<Utc>) -> TodayTaskResponse {
    let is_overdue = row.due_date.is_some_and(|d| d < now) && row.status != "done";
    let is_due_today = !is_overdue
        && row
            .due_date
            .is_some_and(|d| d.date_naive() == now.date_naive());

    let due_display = if is_overdue {
        row.due_date.map(|d| {
            let days = (now - d).num_days();
            if days == 0 {
                "Overdue".to_string()
            } else if days == 1 {
                "Overdue 1d".to_string()
            } else {
                format!("Overdue {days}d")
            }
        })
    } else if is_due_today {
        row.due_date
            .map(|d| format!("Due {}", d.format("%-I:%M %p")))
    } else {
        row.due_date.map(|d| d.format("%b %-d").to_string())
    };

    TodayTaskResponse {
        id: row.id.clone(),
        title: row.title.clone(),
        priority: priority_label(row.priority),
        status: row.status.clone(),
        completed: row.status == "done",
        is_overdue,
        is_due_today,
        due_display,
    }
}

pub(crate) fn objective_to_response(
    row: &ObjectiveRow,
    key_results: Option<Vec<KeyResultResponse>>,
) -> ObjectiveResponse {
    ObjectiveResponse {
        id: row.id.clone(),
        title: row.title.clone(),
        status: row.status.clone(),
        progress: row.progress,
        project_id: row.project_id.clone(),
        key_results,
    }
}

pub(crate) fn kr_to_response(row: &KeyResultRow) -> KeyResultResponse {
    KeyResultResponse {
        id: row.id.clone(),
        title: row.title.clone(),
        progress: row.progress,
        current: row.current_value,
        target: row.target_value.unwrap_or(0.0),
        unit: row.unit.clone().unwrap_or_default(),
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Convert a list of ActionRows to TaskResponses, bulk-fetching subtask counts.
pub(crate) async fn rows_to_tasks(
    repos: &storage::Repos,
    rows: &[ActionRow],
) -> Result<Vec<TaskResponse>, ApiError> {
    let ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
    let counts = repos
        .actions
        .count_children_bulk(&ids)
        .await
        .map_err(super::map_storage_err)?;

    let mut tasks = Vec::with_capacity(rows.len());
    for row in rows {
        let (total, completed) = counts.get(&row.id).copied().unwrap_or((0, 0));
        let mut task = action_to_task(row, total as u32, completed as u32);
        task.status_label = resolve_status_label(repos, row).await?;
        tasks.push(task);
    }
    Ok(tasks)
}

/// Look up the status label for an action row, if it has one.
async fn resolve_status_label(
    repos: &storage::Repos,
    row: &ActionRow,
) -> Result<Option<StatusLabelResponse>, ApiError> {
    match &row.status_label_id {
        Some(label_id) => {
            let label = repos
                .status_workflows
                .get_label(label_id)
                .await
                .map_err(super::map_storage_err)?;
            Ok(label.map(|l| StatusLabelResponse {
                id: l.id,
                workflow_id: l.workflow_id,
                name: l.name,
                color: l.color,
                status_group: l.status_group,
                position: l.position,
            }))
        }
        None => Ok(None),
    }
}

/// Fetch subtask counts for a single row and convert to TaskResponse.
pub(crate) async fn row_to_task(
    repos: &storage::Repos,
    row: &ActionRow,
) -> Result<TaskResponse, ApiError> {
    let (total, completed) = repos
        .actions
        .count_children(&row.id)
        .await
        .map_err(super::map_storage_err)?;
    let mut task = action_to_task(row, total as u32, completed as u32);
    task.status_label = resolve_status_label(repos, row).await?;
    Ok(task)
}

// ── Commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn task_get(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<Option<TaskResponse>, ApiError> {
    match state.repos.actions.get(&id).await.map_err(super::map_storage_err)? {
        Some(row) => Ok(Some(row_to_task(&state.repos, &row).await?)),
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn task_list(
    state: State<'_, Arc<AppCore>>,
    area_id: Option<String>,
    project_id: Option<String>,
    status: Option<String>,
) -> Result<Vec<TaskResponse>, ApiError> {
    let filter = ActionFilter {
        area_id,
        project_id,
        status,
        root_only: true,
        ..Default::default()
    };
    let rows = state
        .repos
        .actions
        .list(&filter)
        .await
        .map_err(super::map_storage_err)?;

    rows_to_tasks(&state.repos, &rows).await
}

#[tauri::command]
pub async fn project_list(
    state: State<'_, Arc<AppCore>>,
    area_id: Option<String>,
) -> Result<Vec<ProjectResponse>, ApiError> {
    let filter = ProjectFilter {
        area_id,
        status: Some("active".to_string()),
        ..Default::default()
    };
    let projects = state
        .repos
        .projects
        .list(&filter)
        .await
        .map_err(super::map_storage_err)?;

    // Fetch all project metadata concurrently to avoid N+1
    let response_futures = projects
        .iter()
        .map(|p| super::projects::build_project_response(&state, p));
    let results = futures_util::future::try_join_all(response_futures).await?;
    Ok(results)
}

#[tauri::command]
pub async fn objective_list(
    state: State<'_, Arc<AppCore>>,
    project_id: Option<String>,
) -> Result<Vec<ObjectiveResponse>, ApiError> {
    let objectives = state
        .repos
        .objectives
        .list(project_id.as_deref(), None)
        .await
        .map_err(super::map_storage_err)?;

    // Fetch all key results concurrently to avoid N+1
    let kr_futures = objectives
        .iter()
        .map(|o| state.repos.key_results.list(Some(&o.id)));
    let all_krs = futures_util::future::try_join_all(kr_futures)
        .await
        .map_err(super::map_storage_err)?;

    let results = objectives
        .iter()
        .zip(all_krs)
        .map(|(o, kr_rows)| {
            let krs = if kr_rows.is_empty() {
                None
            } else {
                Some(kr_rows.iter().map(kr_to_response).collect())
            };
            objective_to_response(o, krs)
        })
        .collect();
    Ok(results)
}

#[tauri::command]
pub async fn task_toggle_complete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<TaskResponse, ApiError> {
    let row = state
        .repos
        .actions
        .get_or_err(&id)
        .await
        .map_err(super::map_storage_err)?;

    let new_status = if row.status == "done" {
        "todo".to_string()
    } else {
        "done".to_string()
    };

    let patch = ActionPatch {
        id: id.clone(),
        status: Some(new_status),
        ..Default::default()
    };

    let updated = state
        .repos
        .actions
        .update(&patch)
        .await
        .map_err(super::map_storage_err)?;

    super::emit_entity_updated(&app, EntityKind::Task, &id);

    row_to_task(&state.repos, &updated).await
}

#[tauri::command]
pub async fn task_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: TaskCreateParams,
) -> Result<TaskResponse, ApiError> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();

    let area_id = match (&params.area_id, &params.parent_id) {
        (Some(aid), _) => aid.clone(),
        (None, Some(pid)) => {
            let parent = state
                .repos
                .actions
                .get_or_err(pid)
                .await
                .map_err(super::map_storage_err)?;
            parent.area_id
        }
        (None, None) => "default".to_string(),
    };

    let row = ActionRow {
        id: id.clone(),
        title: params.title,
        description: None,
        area_id,
        project_id: params.project_id,
        key_result_id: None,
        parent_id: params.parent_id.clone(),
        priority: params.priority,
        due_date: params.due_date.and_then(|d| super::parse_date(&d)),
        tags: params.tags.unwrap_or_default(),
        status: "todo".to_string(),
        focused_at: None,
        focus_deadline: None,
        focus_expired_count: 0,
        created_at: now,
        updated_at: now,
        completed_at: None,
        total_tracked_secs: 0,
        estimated_minutes: None,
        calendar_event_uid: None,
        last_reminded_at: None,
        recurrence_rule: None,
        recurrence_parent_id: None,
        is_template: false,
        next_instance_date: None,
        status_label_id: None,
        position: 0,
    };

    let created = state
        .repos
        .actions
        .add(&row)
        .await
        .map_err(super::map_storage_err)?;

    super::emit_entity_updated(&app, EntityKind::Task, &id);

    // Newly created task has no subtasks yet
    Ok(action_to_task(&created, 0, 0))
}

#[tauri::command]
pub async fn task_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: TaskUpdateParams,
) -> Result<TaskResponse, ApiError> {
    let patch = ActionPatch {
        id: params.id.clone(),
        title: params.title,
        description: params.description,
        priority: params.priority,
        status: params.status,
        due_date: params
            .due_date
            .map(|opt| opt.and_then(|d| super::parse_date(&d))),
        tags: params.tags,
        area_id: params.area_id,
        project_id: params.project_id,
        key_result_id: params.key_result_id,
        ..Default::default()
    };

    let updated = state
        .repos
        .actions
        .update(&patch)
        .await
        .map_err(super::map_storage_err)?;

    super::emit_entity_updated(&app, EntityKind::Task, &params.id);

    row_to_task(&state.repos, &updated).await
}

#[tauri::command]
pub async fn task_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let deleted = state
        .repos
        .actions
        .delete(&id)
        .await
        .map_err(super::map_storage_err)?;

    if deleted {
        super::emit_entity_updated(&app, EntityKind::Task, &id);
    }

    Ok(deleted)
}

#[tauri::command]
pub async fn today_tasks(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<TodayTaskResponse>, ApiError> {
    let now = chrono::Utc::now();
    let start_of_today = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
    let start_of_tomorrow = start_of_today + chrono::Duration::days(1);

    // Run all three queries concurrently — SqlitePool is safe for parallel reads
    let doing_filter = ActionFilter {
        status: Some("doing".to_string()),
        ..Default::default()
    };
    let due_today_filter = ActionFilter {
        due_after: Some(start_of_today),
        due_before: Some(start_of_tomorrow),
        ..Default::default()
    };
    let (doing, due_today, overdue) = tokio::try_join!(
        state.repos.actions.list(&doing_filter),
        state.repos.actions.list(&due_today_filter),
        state.repos.actions.overdue(),
    )
    .map_err(super::map_storage_err)?;

    // Merge + deduplicate by ID
    let mut seen = std::collections::HashSet::new();
    let mut all_rows: Vec<ActionRow> = Vec::new();
    for row in overdue.into_iter().chain(doing).chain(due_today) {
        if row.status != "done" && row.status != "archived" && seen.insert(row.id.clone()) {
            all_rows.push(row);
        }
    }

    // Sort: overdue first, then by priority (P1 first), then by due_date
    all_rows.sort_by(|a, b| {
        let a_overdue = a.due_date.is_some_and(|d| d < now) as u8;
        let b_overdue = b.due_date.is_some_and(|d| d < now) as u8;
        b_overdue
            .cmp(&a_overdue)
            .then(a.priority.unwrap_or(99).cmp(&b.priority.unwrap_or(99)))
            .then(a.due_date.cmp(&b.due_date))
    });

    Ok(all_rows
        .iter()
        .map(|row| action_to_today_task(row, now))
        .collect())
}

#[tauri::command]
pub async fn task_list_children(
    state: State<'_, Arc<AppCore>>,
    parent_id: String,
) -> Result<Vec<TaskResponse>, ApiError> {
    let rows = state
        .repos
        .actions
        .get_children(&parent_id)
        .await
        .map_err(super::map_storage_err)?;

    rows_to_tasks(&state.repos, &rows).await
}
