use desktop_shared::commands::{TaskCreateParams, TaskResponse, TaskUpdateParams};
use desktop_shared::errors::ApiError;
use desktop_shared::types::EntityKind;
use storage::{TaskAttachmentRow, TaskFilter, TaskPatch, TaskRow, TaskTimeEntryRow};
use tracing::warn;

use crate::errors::{map_storage_err, parse_date};
use crate::state::{AppCore, EntityUpdate, HandlerResult};

use super::converters::{resolve_status_label, row_to_task, row_to_task_response, rows_to_tasks};

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn task_get(&self, id: String) -> Result<Option<TaskResponse>, ApiError> {
        match feature_tasks::api::get_task(&self.repos.tasks, &id)
            .await
            .map_err(map_storage_err)?
        {
            Some(row) => Ok(Some(row_to_task(&self.repos, &row).await?)),
            None => Ok(None),
        }
    }

    #[tracing::instrument(skip(self), err)]
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
        let rows = feature_tasks::api::list_tasks(&self.repos.tasks, &filter)
            .await
            .map_err(map_storage_err)?;

        rows_to_tasks(&self.repos, &rows).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn task_create(&self, params: TaskCreateParams) -> HandlerResult<TaskResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        let now: storage::SqlTs = jiff::Timestamp::now().into();

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
            description: params.description,
            area_id,
            project_id: params.project_id,
            key_result_id: None,
            parent_id: params.parent_id.clone(),
            priority: params.priority,
            due_date: params
                .due_date
                .and_then(|d| parse_date(&d))
                .map(|d| d.into()),
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
            energy_level: params.energy_level,
            estimated_focus_blocks: None,
            actual_minutes: None,
            complexity_score: None,
            completed: false,
            objective_id: None,
            scheduled_start: None,
            scheduled_end: None,
        };

        let created = feature_tasks::api::create_task(&self.repos.tasks, &row)
            .await
            .map_err(map_storage_err)?;

        let updates = vec![EntityUpdate {
            kind: EntityKind::Task,
            id: id.clone(),
        }];

        // Emit domain events for timeline tracking + tree rebuild
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

    #[tracing::instrument(skip(self))]
    pub async fn task_update(
        &self,
        params: TaskUpdateParams,
        _actor: Option<String>,
    ) -> HandlerResult<TaskResponse> {
        // Capture previous due_date before update so we can detect deferrals.
        let previous_due: Option<jiff::civil::Date> = if params.due_date.is_some() {
            self.repos
                .tasks
                .get(&params.id)
                .await
                .map_err(map_storage_err)?
                .and_then(|r| r.due_date)
                .map(|ts| {
                    let ts: jiff::Timestamp = ts.into();
                    ts.to_zoned(jiff::tz::TimeZone::UTC).date()
                })
        } else {
            None
        };

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
            energy_level: params.energy_level.map(Some),
            estimated_minutes: params.estimated_minutes,
            scheduled_start: params
                .scheduled_start
                .map(|opt| opt.and_then(|d| common::parse_datetime_jiff(&d, "UTC"))),
            scheduled_end: params
                .scheduled_end
                .map(|opt| opt.and_then(|d| common::parse_datetime_jiff(&d, "UTC"))),
            ..Default::default()
        };

        let updated = feature_tasks::api::update_task(&self.repos.tasks, &patch)
            .await
            .map_err(map_storage_err)?;

        // Emit TaskDeferred when due_date was pushed to a later date.
        if let Some(new_due_ts) = &updated.due_date {
            let new_due: jiff::civil::Date = {
                let ts: jiff::Timestamp = (*new_due_ts).into();
                ts.to_zoned(jiff::tz::TimeZone::UTC).date()
            };
            let is_deferred = match previous_due {
                Some(prev) => new_due > prev,
                // No previous due date → moving from "no deadline" to a future date
                // is not a deferral — only pushes of an existing date count.
                None => false,
            };
            if is_deferred {
                if let Ok(bus) = self.domain_event_bus() {
                    bus.publish(bus::DomainEvent::TaskDeferred {
                        task_id: params.id.clone(),
                        times_deferred: 1,
                    });
                }
            }
        }

        let updates = vec![EntityUpdate {
            kind: EntityKind::Task,
            id: params.id.clone(),
        }];

        let response = row_to_task(&self.repos, &updated).await?;
        Ok((response, updates))
    }

    #[tracing::instrument(skip(self))]
    pub async fn task_delete(&self, id: String) -> HandlerResult<bool> {
        let deleted = feature_tasks::api::delete_task(&self.repos.tasks, &id)
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

    #[tracing::instrument(skip(self))]
    pub async fn task_toggle_complete(&self, id: String) -> HandlerResult<TaskResponse> {
        let updated = feature_tasks::api::toggle_complete(&self.repos.tasks, &id)
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

                // Emit EstimationRecorded when all three values are present.
                if let (Some(est_mins), Some(act_mins), Some(dev)) =
                    (estimated_mins, actual_mins, deviation_pct)
                {
                    bus.publish(bus::DomainEvent::EstimationRecorded {
                        task_id: id.clone(),
                        estimated_mins: est_mins as u32,
                        actual_mins: act_mins as u32,
                        deviation_pct: dev,
                    });
                }
            }
        }

        let updates = vec![EntityUpdate {
            kind: EntityKind::Task,
            id,
        }];

        let response = row_to_task(&self.repos, &updated).await?;
        Ok((response, updates))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn task_list_children(
        &self,
        parent_id: String,
    ) -> Result<Vec<TaskResponse>, ApiError> {
        let rows = feature_tasks::api::list_children(&self.repos.tasks, &parent_id)
            .await
            .map_err(map_storage_err)?;

        rows_to_tasks(&self.repos, &rows).await
    }

    // ── Dependencies ────────────────────────────────────────────────────

    #[tracing::instrument(skip(self), err)]
    pub async fn task_add_dependency(
        &self,
        task_id: String,
        blocker_id: String,
    ) -> Result<(), ApiError> {
        feature_tasks::api::add_dependency(&self.repos.tasks, &task_id, &blocker_id)
            .await
            .map_err(map_storage_err)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn task_list_dependencies(
        &self,
        task_id: String,
    ) -> Result<Vec<TaskResponse>, ApiError> {
        let rows = feature_tasks::api::list_dependencies(&self.repos.tasks, &task_id)
            .await
            .map_err(map_storage_err)?;

        rows_to_tasks(&self.repos, &rows).await
    }

    // ── Attachments ─────────────────────────────────────────────────────

    #[tracing::instrument(skip(self), err)]
    pub async fn task_add_attachment(
        &self,
        task_id: String,
        attachment_type: String,
        value: String,
        title: Option<String>,
    ) -> Result<TaskAttachmentRow, ApiError> {
        feature_tasks::api::add_attachment(
            &self.repos.tasks,
            &task_id,
            &attachment_type,
            &value,
            title.as_deref(),
        )
        .await
        .map_err(map_storage_err)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn task_list_attachments(
        &self,
        task_id: String,
    ) -> Result<Vec<TaskAttachmentRow>, ApiError> {
        feature_tasks::api::list_attachments(&self.repos.tasks, &task_id)
            .await
            .map_err(map_storage_err)
    }

    // ── Time entries ────────────────────────────────────────────────────

    #[tracing::instrument(skip(self), err)]
    pub async fn task_add_time_entry(
        &self,
        task_id: String,
        started_at: jiff::Timestamp,
        duration_secs: Option<i64>,
        note: Option<String>,
    ) -> Result<TaskTimeEntryRow, ApiError> {
        feature_tasks::api::add_time_entry(
            &self.repos.tasks,
            &task_id,
            started_at,
            duration_secs,
            note.as_deref(),
        )
        .await
        .map_err(map_storage_err)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn task_list_time_entries(
        &self,
        task_id: String,
    ) -> Result<Vec<TaskTimeEntryRow>, ApiError> {
        feature_tasks::api::list_time_entries(&self.repos.tasks, &task_id)
            .await
            .map_err(map_storage_err)
    }
}
