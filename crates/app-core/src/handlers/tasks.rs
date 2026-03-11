use chrono::{DateTime, Utc};
use desktop_shared::commands::{
    KeyResultResponse, ObjectiveResponse, ProjectResponse, StatusLabelResponse, TaskCreateParams,
    TaskResponse, TaskUpdateParams, TodayTaskResponse,
};
use desktop_shared::errors::ApiError;
use desktop_shared::types::EntityKind;
use storage::{KeyResultRow, ObjectiveRow, ProjectFilter, TaskFilter, TaskPatch, TaskRow};
use tracing::warn;

use crate::errors::{map_storage_err, parse_date};
use crate::state::{AppCore, EntityUpdate, HandlerResult};

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
    }
}

pub fn action_to_today_task(row: &TaskRow, now: DateTime<Utc>) -> TodayTaskResponse {
    let is_overdue = row.due_date.is_some_and(|d| d < now) && !row.completed;
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
async fn resolve_status_label(
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

// ── Handler methods ─────────────────────────────────────────────────────

impl AppCore {
    pub async fn task_get(&self, id: String) -> Result<Option<TaskResponse>, ApiError> {
        match self.repos.tasks.get(&id).await.map_err(map_storage_err)? {
            Some(row) => Ok(Some(row_to_task(&self.repos, &row).await?)),
            None => Ok(None),
        }
    }

    pub async fn task_list(
        &self,
        area_id: Option<String>,
        project_id: Option<String>,
        status: Option<String>,
    ) -> Result<Vec<TaskResponse>, ApiError> {
        let filter = TaskFilter {
            area_id,
            project_id,
            status,
            root_only: true,
            ..Default::default()
        };
        let rows = self
            .repos
            .tasks
            .list(&filter)
            .await
            .map_err(map_storage_err)?;

        rows_to_tasks(&self.repos, &rows).await
    }

    pub async fn task_create(&self, params: TaskCreateParams) -> HandlerResult<TaskResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        let area_id = match (&params.area_id, &params.parent_id) {
            (Some(aid), _) => aid.clone(),
            (None, Some(pid)) => {
                let parent = self
                    .repos
                    .tasks
                    .get_or_err(pid)
                    .await
                    .map_err(map_storage_err)?;
                parent.area_id
            }
            (None, None) => {
                return Err(ApiError {
                    code: "VALIDATION".to_string(),
                    message: "area_id is required when creating a top-level task".to_string(),
                });
            }
        };

        // Auto-assign status_label_id if not provided
        let status_label_id = match params.status_label_id {
            Some(id) => Some(id),
            None => {
                let wf = self
                    .repos
                    .status_workflows
                    .get_global_default()
                    .await
                    .map_err(|e| {
                        warn!(error = %e, "failed to fetch global default workflow during task creation");
                        e
                    })
                    .ok()
                    .flatten();

                match wf {
                    Some(wf) => self
                        .repos
                        .status_workflows
                        .find_label_by_group(&wf.id, "not_started")
                        .await
                        .map_err(|e| {
                            warn!(workflow_id = %wf.id, error = %e, "failed to find not_started label");
                            e
                        })
                        .ok()
                        .flatten()
                        .map(|l| l.id),
                    None => None,
                }
            }
        };

        let row = TaskRow {
            id: id.clone(),
            title: params.title,
            description: None,
            area_id,
            project_id: params.project_id,
            key_result_id: None,
            parent_id: params.parent_id.clone(),
            priority: params.priority,
            due_date: params.due_date.and_then(|d| parse_date(&d)),
            tags: params.tags.unwrap_or_default(),
            status: "todo".to_string(),
            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: now,
            updated_at: now,
            completed_at: None,
            total_tracked_secs: 0,
            estimated_minutes: params.estimated_minutes,
            calendar_event_uid: None,
            last_reminded_at: None,
            recurrence_rule: None,
            recurrence_parent_id: None,
            is_template: false,
            next_instance_date: None,
            status_label_id,
            position: 0,
            group_id: params.group_id,
            task_type: params.task_type.unwrap_or_else(|| "manual".to_string()),
            acceptance_criteria: params.acceptance_criteria,
            agent_config: None,
            execution_state: "idle".to_string(),
            spawned_execution_id: None,
            context_snapshot: None,
            energy_level: params.energy_level,
            estimated_focus_blocks: None,
            actual_minutes: None,
            complexity_score: None,
            completed: false,
            objective_id: None,
        };

        let created = self.repos.tasks.add(&row).await.map_err(map_storage_err)?;

        let updates = vec![EntityUpdate {
            kind: EntityKind::Task,
            id: id.clone(),
        }];

        // Emit domain event for timeline tracking
        if let Ok(bus) = self.domain_event_bus() {
            bus.publish(bus::DomainEvent::TaskCreated {
                task_id: id.clone(),
                project: created.project_id.clone(),
                estimate_mins: created.estimated_minutes.map(|m| m as i64),
                task_type: created.task_type.clone(),
            });
        }

        // Newly created task has no subtasks yet; resolve status label for the response
        let mut task = row_to_task_response(&created, 0, 0);
        task.status_label = resolve_status_label(&self.repos, &created).await?;
        Ok((task, updates))
    }

    pub async fn task_update(&self, params: TaskUpdateParams) -> HandlerResult<TaskResponse> {
        let patch = TaskPatch {
            id: params.id.clone(),
            title: params.title,
            description: params.description,
            priority: params.priority,
            status: params.status,
            due_date: params.due_date.map(|opt| opt.and_then(|d| parse_date(&d))),
            tags: params.tags,
            area_id: params.area_id,
            project_id: params.project_id,
            key_result_id: params.key_result_id,
            status_label_id: params.status_label_id,
            group_id: params.group_id,
            task_type: params.task_type,
            acceptance_criteria: params.acceptance_criteria,
            energy_level: params.energy_level.map(Some),
            execution_state: params.execution_state,
            estimated_minutes: params.estimated_minutes,
            ..Default::default()
        };

        let updated = self
            .repos
            .tasks
            .update(&patch)
            .await
            .map_err(map_storage_err)?;

        let updates = vec![EntityUpdate {
            kind: EntityKind::Task,
            id: params.id,
        }];

        let response = row_to_task(&self.repos, &updated).await?;
        Ok((response, updates))
    }

    pub async fn task_delete(&self, id: String) -> HandlerResult<bool> {
        let deleted = self
            .repos
            .tasks
            .delete(&id)
            .await
            .map_err(map_storage_err)?;

        let updates = if deleted {
            vec![EntityUpdate {
                kind: EntityKind::Task,
                id,
            }]
        } else {
            vec![]
        };

        Ok((deleted, updates))
    }

    pub async fn task_toggle_complete(&self, id: String) -> HandlerResult<TaskResponse> {
        let row = self
            .repos
            .tasks
            .get_or_err(&id)
            .await
            .map_err(map_storage_err)?;

        let new_status = if row.completed {
            "todo".to_string()
        } else {
            "done".to_string()
        };

        let patch = TaskPatch {
            id: id.clone(),
            status: Some(new_status),
            completed: Some(!row.completed),
            ..Default::default()
        };

        let updated = self
            .repos
            .tasks
            .update(&patch)
            .await
            .map_err(map_storage_err)?;

        // Emit domain event when completing
        if updated.completed {
            if let Ok(bus) = self.domain_event_bus() {
                let actual_mins = updated.actual_minutes.map(|m| m as i64).or_else(|| {
                    if updated.total_tracked_secs > 0 {
                        Some(updated.total_tracked_secs / 60)
                    } else {
                        None
                    }
                });
                let estimated_mins = updated.estimated_minutes.map(|m| m as i64);
                let deviation_pct = match (actual_mins, estimated_mins) {
                    (Some(a), Some(e)) if e > 0 => Some(((a as f64) / (e as f64) - 1.0) * 100.0),
                    _ => None,
                };
                bus.publish(bus::DomainEvent::TaskCompleted {
                    task_id: id.clone(),
                    actual_duration_mins: actual_mins,
                    estimated_duration_mins: estimated_mins,
                    deviation_pct,
                });
            }
        }

        let updates = vec![EntityUpdate {
            kind: EntityKind::Task,
            id,
        }];

        let response = row_to_task(&self.repos, &updated).await?;
        Ok((response, updates))
    }

    pub async fn task_list_children(
        &self,
        parent_id: String,
    ) -> Result<Vec<TaskResponse>, ApiError> {
        let rows = self
            .repos
            .tasks
            .get_children(&parent_id)
            .await
            .map_err(map_storage_err)?;

        rows_to_tasks(&self.repos, &rows).await
    }

    pub async fn today_tasks(&self) -> Result<Vec<TodayTaskResponse>, ApiError> {
        let now = chrono::Utc::now();
        let start_of_today = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let start_of_tomorrow = start_of_today + chrono::Duration::days(1);

        // Run all three queries concurrently — SqlitePool is safe for parallel reads
        let doing_filter = TaskFilter {
            status: Some("doing".to_string()),
            ..Default::default()
        };
        let due_today_filter = TaskFilter {
            due_after: Some(start_of_today),
            due_before: Some(start_of_tomorrow),
            ..Default::default()
        };
        let (doing, due_today, overdue) = tokio::try_join!(
            self.repos.tasks.list(&doing_filter),
            self.repos.tasks.list(&due_today_filter),
            self.repos.tasks.overdue(),
        )
        .map_err(map_storage_err)?;

        // Merge + deduplicate by ID
        let mut seen = std::collections::HashSet::new();
        let mut all_rows: Vec<TaskRow> = Vec::new();
        for row in overdue.into_iter().chain(doing).chain(due_today) {
            if !row.completed && row.status != "archived" && seen.insert(row.id.clone()) {
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

    pub async fn project_list_for_tasks(
        &self,
        area_id: Option<String>,
    ) -> Result<Vec<ProjectResponse>, ApiError> {
        let filter = ProjectFilter {
            area_id,
            status: Some("active".to_string()),
            ..Default::default()
        };
        let projects = self
            .repos
            .projects
            .list(&filter)
            .await
            .map_err(map_storage_err)?;

        // Fetch all project metadata concurrently to avoid N+1
        let response_futures = projects
            .iter()
            .map(|p| super::projects::build_project_response(self, p));
        let results = futures_util::future::try_join_all(response_futures).await?;
        Ok(results)
    }

    pub async fn objective_list_for_tasks(
        &self,
        project_id: Option<String>,
    ) -> Result<Vec<ObjectiveResponse>, ApiError> {
        let objectives = self
            .repos
            .objectives
            .list(project_id.as_deref(), None)
            .await
            .map_err(map_storage_err)?;

        // Fetch all key results concurrently to avoid N+1
        let kr_futures = objectives
            .iter()
            .map(|o| self.repos.key_results.list(Some(&o.id)));
        let all_krs = futures_util::future::try_join_all(kr_futures)
            .await
            .map_err(map_storage_err)?;

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
}
