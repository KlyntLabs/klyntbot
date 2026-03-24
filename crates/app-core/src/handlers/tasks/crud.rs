use chrono::{DateTime, Utc};
use desktop_shared::commands::{TaskCreateParams, TaskResponse, TaskUpdateParams};
use desktop_shared::errors::ApiError;
use desktop_shared::types::EntityKind;
use storage::{TaskAttachmentRow, TaskFilter, TaskPatch, TaskRow, TaskTimeEntryRow};
use tracing::warn;

use crate::errors::{map_storage_err, parse_date};
use crate::state::{AppCore, EntityUpdate, HandlerResult};

use super::converters::{resolve_status_label, row_to_task, row_to_task_response, rows_to_tasks};

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
            scheduled_start: None,
            scheduled_end: None,
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

    pub async fn task_update(
        &self,
        params: TaskUpdateParams,
        actor: Option<String>,
    ) -> HandlerResult<TaskResponse> {
        // Fetch old task before applying the patch so we can diff
        let old_task = self
            .repos
            .tasks
            .get(&params.id)
            .await
            .map_err(map_storage_err)?;

        // Capture fields needed for diffing before they move into the patch
        let task_id = params.id.clone();
        let new_status = params.status.clone();
        let new_priority = params.priority;
        let new_title = params.title.clone();

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
            scheduled_start: params
                .scheduled_start
                .map(|opt| opt.and_then(|d| common::parse_datetime(&d, "UTC"))),
            scheduled_end: params
                .scheduled_end
                .map(|opt| opt.and_then(|d| common::parse_datetime(&d, "UTC"))),
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
            id: task_id.clone(),
        }];

        // Diff old vs new and emit domain events
        if let Some(ref old) = old_task {
            if let Ok(bus) = self.domain_event_bus() {
                if let Some(ref status) = new_status {
                    if old.status != *status {
                        bus.publish(bus::DomainEvent::TaskStatusChanged {
                            task_id: task_id.clone(),
                            from: old.status.clone(),
                            to: status.clone(),
                            actor: actor.clone(),
                        });
                    }
                }

                if let Some(ref priority_opt) = new_priority {
                    let old_str = old.priority.map(|p| p.to_string()).unwrap_or_default();
                    let new_str = priority_opt.map(|p| p.to_string()).unwrap_or_default();
                    if old_str != new_str {
                        bus.publish(bus::DomainEvent::TaskPriorityChanged {
                            task_id: task_id.clone(),
                            from: old_str,
                            to: new_str,
                            actor: actor.clone(),
                        });
                    }
                }

                if let Some(ref title) = new_title {
                    if old.title != *title {
                        bus.publish(bus::DomainEvent::TaskFieldUpdated {
                            task_id: task_id.clone(),
                            field: "title".to_string(),
                            from: old.title.clone(),
                            to: title.clone(),
                            actor,
                        });
                    }
                }
            }
        }

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
                let actual_mins = updated.actual_minutes.map(|m| m as i64).or(
                    if updated.total_tracked_secs > 0 {
                        Some(updated.total_tracked_secs / 60)
                    } else {
                        None
                    },
                );
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

    // ── Dependencies ────────────────────────────────────────────────────

    pub async fn task_add_dependency(
        &self,
        task_id: String,
        blocker_id: String,
    ) -> Result<(), ApiError> {
        self.repos
            .tasks
            .add_dependency(&task_id, &blocker_id, "blocks")
            .await
            .map_err(map_storage_err)
    }

    pub async fn task_list_dependencies(
        &self,
        task_id: String,
    ) -> Result<Vec<TaskResponse>, ApiError> {
        let rows = self
            .repos
            .tasks
            .get_blockers(&task_id)
            .await
            .map_err(map_storage_err)?;

        rows_to_tasks(&self.repos, &rows).await
    }

    // ── Attachments ─────────────────────────────────────────────────────

    pub async fn task_add_attachment(
        &self,
        task_id: String,
        attachment_type: String,
        value: String,
        title: Option<String>,
    ) -> Result<TaskAttachmentRow, ApiError> {
        self.repos
            .tasks
            .add_attachment(
                &task_id,
                &attachment_type,
                &value,
                title.as_deref(),
                &[],
                "user",
            )
            .await
            .map_err(map_storage_err)
    }

    pub async fn task_list_attachments(
        &self,
        task_id: String,
    ) -> Result<Vec<TaskAttachmentRow>, ApiError> {
        self.repos
            .tasks
            .list_attachments(&task_id)
            .await
            .map_err(map_storage_err)
    }

    // ── Time entries ────────────────────────────────────────────────────

    pub async fn task_add_time_entry(
        &self,
        task_id: String,
        started_at: DateTime<Utc>,
        duration_secs: Option<i64>,
        note: Option<String>,
    ) -> Result<TaskTimeEntryRow, ApiError> {
        self.repos
            .tasks
            .add_time_entry(
                &task_id,
                "user",
                started_at,
                duration_secs,
                note.as_deref(),
                None,
            )
            .await
            .map_err(map_storage_err)
    }

    pub async fn task_list_time_entries(
        &self,
        task_id: String,
    ) -> Result<Vec<TaskTimeEntryRow>, ApiError> {
        self.repos
            .tasks
            .list_time_entries(&task_id)
            .await
            .map_err(map_storage_err)
    }
}
