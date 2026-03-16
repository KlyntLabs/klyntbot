use chrono::{DateTime, Utc};
use desktop_shared::commands::{
    KeyResultResponse, ObjectiveResponse, StatusLabelResponse, TaskResponse, TodayTaskResponse,
};
use desktop_shared::errors::ApiError;
use storage::{KeyResultRow, ObjectiveRow, TaskRow};

use crate::errors::map_storage_err;

// ── Row → Response converters ───────────────────────────────────────────

pub fn priority_label(p: Option<i16>) -> Option<String> {
    p.map(|v| format!("P{v}"))
}

pub fn row_to_task_response(
    row: &TaskRow,
    subtask_count: u32,
    subtask_completed_count: u32,
) -> TaskResponse {
    TaskResponse {
        id: row.id.clone(),
        title: row.title.clone(),
        completed: row.completed,
        priority: priority_label(row.priority),
        status: row.status.clone(),
        due_date: row.due_date.map(|d| d.format("%Y-%m-%d").to_string()),
        tags: row.tags.clone(),
        project_id: row.project_id.clone(),
        area_id: row.area_id.clone(),
        objective_id: row.objective_id.clone(),
        description: row.description.clone(),
        parent_id: row.parent_id.clone(),
        subtask_count,
        subtask_completed_count,
        status_label_id: row.status_label_id.clone(),
        status_label: None,
        group_id: row.group_id.clone(),
        task_type: Some(row.task_type.clone()),
        execution_state: Some(row.execution_state.clone()),
        energy_level: row.energy_level.clone(),
        acceptance_criteria: row.acceptance_criteria.clone(),
        estimated_minutes: row.estimated_minutes,
        actual_minutes: row.actual_minutes,
        complexity_score: row.complexity_score,
        total_tracked_secs: Some(row.total_tracked_secs),
        focused_at: row.focused_at.map(|dt| dt.to_rfc3339()),
        created_at: Some(row.created_at.to_rfc3339()),
        updated_at: Some(row.updated_at.to_rfc3339()),
    }
}

pub fn action_to_today_task(row: &TaskRow, now: DateTime<Utc>) -> TodayTaskResponse {
    let local_today = now.with_timezone(&chrono::Local).date_naive();
    let is_overdue = row.due_date.is_some_and(|d| d < now) && !row.completed;
    let is_due_today = !is_overdue
        && row
            .due_date
            .is_some_and(|d| d.with_timezone(&chrono::Local).date_naive() == local_today);

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
        row.due_date.map(|d| {
            format!(
                "Due {}",
                d.with_timezone(&chrono::Local).format("%-I:%M %p")
            )
        })
    } else {
        row.due_date.map(|d| d.format("%b %-d").to_string())
    };

    TodayTaskResponse {
        id: row.id.clone(),
        title: row.title.clone(),
        priority: priority_label(row.priority),
        status: row.status.clone(),
        completed: row.completed,
        is_overdue,
        is_due_today,
        due_display,
    }
}

pub fn objective_to_response(
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

pub fn kr_to_response(row: &KeyResultRow) -> KeyResultResponse {
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

/// Convert a list of TaskRows to TaskResponses, bulk-fetching subtask counts and status labels.
pub async fn rows_to_tasks(
    repos: &storage::Repos,
    rows: &[TaskRow],
) -> Result<Vec<TaskResponse>, ApiError> {
    let ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
    let counts = repos
        .tasks
        .count_children_bulk(&ids)
        .await
        .map_err(map_storage_err)?;

    // Batch-load all unique status labels to avoid N+1
    let label_ids: Vec<&str> = rows
        .iter()
        .filter_map(|r| r.status_label_id.as_deref())
        .collect::<std::collections::HashSet<&str>>()
        .into_iter()
        .collect();
    let labels = repos
        .status_workflows
        .get_labels_by_ids(&label_ids)
        .await
        .map_err(map_storage_err)?;
    let label_map: std::collections::HashMap<&str, _> =
        labels.iter().map(|l| (l.id.as_str(), l)).collect();

    let mut tasks = Vec::with_capacity(rows.len());
    for row in rows {
        let (total, completed) = counts.get(&row.id).copied().unwrap_or((0, 0));
        let mut task = row_to_task_response(row, total as u32, completed as u32);
        task.status_label = row
            .status_label_id
            .as_deref()
            .and_then(|id| label_map.get(id))
            .map(|l| StatusLabelResponse {
                id: l.id.clone(),
                workflow_id: l.workflow_id.clone(),
                name: l.name.clone(),
                color: l.color.clone(),
                status_group: l.status_group.clone(),
                position: l.position,
            });
        tasks.push(task);
    }
    Ok(tasks)
}

/// Look up the status label for a task row, if it has one.
pub(super) async fn resolve_status_label(
    repos: &storage::Repos,
    row: &TaskRow,
) -> Result<Option<StatusLabelResponse>, ApiError> {
    match &row.status_label_id {
        Some(label_id) => {
            let label = repos
                .status_workflows
                .get_label(label_id)
                .await
                .map_err(map_storage_err)?;
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
pub async fn row_to_task(repos: &storage::Repos, row: &TaskRow) -> Result<TaskResponse, ApiError> {
    let (total, completed) = repos
        .tasks
        .count_children(&row.id)
        .await
        .map_err(map_storage_err)?;
    let mut task = row_to_task_response(row, total as u32, completed as u32);
    task.status_label = resolve_status_label(repos, row).await?;
    Ok(task)
}
