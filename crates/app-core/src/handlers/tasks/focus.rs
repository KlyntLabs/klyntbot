use chrono::Utc;
use desktop_shared::commands::TaskResponse;
use desktop_shared::errors::ApiError;
use desktop_shared::types::EntityKind;
use feature_tasks::types::ActiveTaskFocus;

use crate::errors::map_storage_err;
use crate::state::{AppCore, EntityUpdate, HandlerResult};

use super::converters::row_to_task;

impl AppCore {
    /// Start a focus session on the given task.
    ///
    /// If another task is already focused, it is auto-ended first.
    pub async fn start_focus(&self, task_id: String) -> HandlerResult<TaskResponse> {
        // Step 1: Read task or return not-found error.
        let task_row = self
            .repos
            .tasks
            .get(&task_id)
            .await
            .map_err(map_storage_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", format!("task not found: {task_id}")))?;

        let mut updates: Vec<EntityUpdate> = vec![];

        // Step 2: Auto-end any existing focus session on a different task.
        let prev_session = self.active_task_focus.lock().unwrap().clone();
        if let Some(prev) = prev_session {
            if prev.task_id != task_id {
                let (_, prev_updates) = self.end_focus_inner(&prev).await?;
                updates.extend(prev_updates);
            }
        }

        // Step 3: Set focused_at in DB (max_slots=1 enforces one focused task at a time).
        self.repos
            .tasks
            .focus(&task_id, 1, None)
            .await
            .map_err(map_storage_err)?;

        // Step 4: Store ActiveTaskFocus in memory.
        let energy_level = task_row.energy_level.clone();
        {
            let mut lock = self.active_task_focus.lock().unwrap();
            *lock = Some(ActiveTaskFocus {
                task_id: task_id.clone(),
                started_at: Utc::now(),
                energy_level: energy_level
                    .as_deref()
                    .and_then(|s| s.parse().ok()),
            });
        }

        // Step 5: Emit TaskFocusStarted.
        if let Some(bus) = &self.domain_event_bus {
            bus.publish(bus::DomainEvent::TaskFocusStarted {
                task_id: task_id.clone(),
                energy_level: energy_level
                    .as_deref()
                    .unwrap_or("medium")
                    .to_string(),
            });
        }

        updates.push(EntityUpdate {
            kind: EntityKind::Task,
            id: task_id.clone(),
        });

        // Reload row so response reflects updated focused_at.
        let updated_row = self
            .repos
            .tasks
            .get(&task_id)
            .await
            .map_err(map_storage_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", format!("task not found: {task_id}")))?;
        let response = row_to_task(&self.repos, &updated_row).await?;

        Ok((response, updates))
    }

    /// End the active focus session.
    ///
    /// Returns `Ok(None)` if no session is active (no-op).
    pub async fn end_focus(
        &self,
        _task_id: String,
    ) -> Result<Option<(TaskResponse, Vec<EntityUpdate>)>, ApiError> {
        let session = self.active_task_focus.lock().unwrap().clone();
        let Some(focus) = session else {
            return Ok(None);
        };
        let result = self.end_focus_inner(&focus).await?;
        Ok(Some(result))
    }

    pub(crate) async fn end_focus_inner(
        &self,
        focus: &ActiveTaskFocus,
    ) -> HandlerResult<TaskResponse> {
        let task_id = &focus.task_id;

        // Read full task BEFORE clearing state (needed for estimated_minutes).
        let task_row = self
            .repos
            .tasks
            .get(task_id)
            .await
            .map_err(map_storage_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", format!("task not found: {task_id}")))?;
        let task_response = row_to_task(&self.repos, &task_row).await?;

        let duration_secs = Utc::now()
            .signed_duration_since(focus.started_at)
            .num_seconds()
            .max(0) as u64;

        // Clear focused_at in DB.
        self.repos
            .tasks
            .unfocus(task_id)
            .await
            .map_err(map_storage_err)?;

        // Clear AppCore session state.
        {
            let mut lock = self.active_task_focus.lock().unwrap();
            if lock
                .as_ref()
                .map(|f| f.task_id.as_str())
                .is_some_and(|id| id == task_id)
            {
                *lock = None;
            }
        }

        if let Some(bus) = &self.domain_event_bus {
            bus.publish(bus::DomainEvent::TaskFocusEnded {
                task_id: task_id.clone(),
                duration_secs,
            });

            // Emit EstimationRecorded only when estimated_minutes is set.
            if let Some(estimated_mins) = task_response.estimated_minutes {
                if estimated_mins > 0 {
                    let actual_mins = (duration_secs / 60) as u32;
                    let deviation_pct = (actual_mins as f64 - estimated_mins as f64)
                        / estimated_mins as f64
                        * 100.0;
                    bus.publish(bus::DomainEvent::EstimationRecorded {
                        task_id: task_id.clone(),
                        estimated_mins: estimated_mins as u32,
                        actual_mins,
                        deviation_pct,
                    });
                }
            }
        }

        let updates = vec![EntityUpdate {
            kind: EntityKind::Task,
            id: task_id.clone(),
        }];
        Ok((task_response, updates))
    }
}
