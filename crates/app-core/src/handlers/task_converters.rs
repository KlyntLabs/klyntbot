//! Converter utilities for task-related response types.
//!
//! Extracted from the removed `handlers::tasks::converters` module.
//! Only the functions still referenced by other handler modules are kept.

use desktop_shared::commands::{KeyResultResponse, ObjectiveResponse, TaskResponse};
use desktop_shared::errors::ApiError;
use storage::{KeyResultRow, ObjectiveRow, TaskRow};

use crate::errors::map_storage_err;

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
        scheduled_start: row.scheduled_start.map(|dt| dt.to_rfc3339()),
        scheduled_end: row.scheduled_end.map(|dt| dt.to_rfc3339()),
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

    Ok(rows
        .iter()
        .map(|r| {
            let (total, completed) = counts.get(r.id.as_str()).copied().unwrap_or((0, 0));
            let mut resp = row_to_task_response(r, total as u32, completed as u32);
            if let Some(label_id) = &r.status_label_id {
                if let Some(label) = label_map.get(label_id.as_str()) {
                    resp.status_label = Some(desktop_shared::commands::StatusLabelResponse {
                        id: label.id.clone(),
                        workflow_id: label.workflow_id.clone(),
                        name: label.name.clone(),
                        color: label.color.clone(),
                        status_group: label.status_group.clone(),
                        position: label.position,
                    });
                }
            }
            resp
        })
        .collect())
}

/// Build a single TaskResponse from a TaskRow, fetching subtask counts.
pub async fn row_to_task(repos: &storage::Repos, row: TaskRow) -> Result<TaskResponse, ApiError> {
    let (total, completed) = repos
        .tasks
        .count_children(&row.id)
        .await
        .map_err(map_storage_err)?;
    let mut resp = row_to_task_response(&row, total as u32, completed as u32);
    if let Some(label_id) = &row.status_label_id {
        if let Ok(Some(label)) = repos.status_workflows.get_label(label_id).await {
            resp.status_label = Some(desktop_shared::commands::StatusLabelResponse {
                id: label.id,
                workflow_id: label.workflow_id,
                name: label.name,
                color: label.color,
                status_group: label.status_group,
                position: label.position,
            });
        }
    }
    Ok(resp)
}

// ── AppCore query methods ───────────────────────────────────────────────

impl crate::state::AppCore {
    /// Return the next upcoming task (earliest non-completed task with a future due date).
    pub async fn next_upcoming_task(&self) -> Option<TaskRow> {
        let now = chrono::Utc::now();
        let filter = storage::TaskFilter {
            due_after: Some(now),
            limit: Some(1),
            ..Default::default()
        };
        let tasks = self.repos.tasks.list(&filter).await.ok()?;
        tasks.into_iter().find(|t| !t.completed)
    }
}
