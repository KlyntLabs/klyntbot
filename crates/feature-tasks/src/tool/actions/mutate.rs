//! Update, complete, and delete action handlers.

use chrono::Utc;
use common::{Result, ToolError};
use tools_core::ParamExtractor;
use tracing::warn;

use bus::DomainEvent;

use super::super::TaskTool;
use crate::types::Task;
use storage::TaskPatch;

impl TaskTool {
    pub(crate) async fn handle_update(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;

        // Capture old values for activity logging
        let old_row = if self.config.auto_log_activity {
            self.repo.get(id).await?
        } else {
            None
        };

        let patch = TaskPatch {
            id: id.to_string(),
            title: p.optional_str("title")?.map(String::from),
            description: p.optional_str("description")?.map(|s| Some(s.to_string())),
            priority: p.optional_u64("priority")?.map(|v| Some(v as i16)),
            due_date: p.optional_str("due_date")?.map(|s| {
                if s.is_empty() || s == "null" {
                    None
                } else {
                    common::parse_datetime(s, &self.timezone)
                }
            }),
            tags: p.optional_array("tags")?.map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            }),
            status: p.optional_str("status")?.map(String::from),
            last_reminded_at: None,
            calendar_event_uid: None,
            next_instance_date: None,
            estimated_minutes: p.optional_u64("estimated_minutes")?.map(|v| Some(v as i32)),
            recurrence_rule: None,
            area_id: p.optional_str("area_id")?.map(String::from),
            project_id: p.optional_str("project_id")?.map(|s| Some(s.to_string())),
            key_result_id: p
                .optional_str("key_result_id")?
                .map(|s| Some(s.to_string())),
            status_label_id: p
                .optional_str("status_label_id")?
                .map(|s| Some(s.to_string())),
            position: p.optional_u64("position")?.map(|v| v as i32),
            group_id: p.optional_str("group_id")?.map(|s| Some(s.to_string())),
            // New agentic fields
            task_type: p.optional_str("task_type")?.map(String::from),
            acceptance_criteria: p
                .optional_str("acceptance_criteria")?
                .map(|s| Some(s.to_string())),
            agent_config: p.optional_str("agent_config")?.map(|s| Some(s.to_string())),
            execution_state: None,
            spawned_execution_id: None,
            energy_level: p.optional_str("energy_level")?.map(|s| Some(s.to_string())),
            complexity_score: None,
            completed: None,
            actual_minutes: None,
            objective_id: p.optional_str("objective_id")?.map(|s| Some(s.to_string())),
        };

        match self.repo.update(&patch).await {
            Ok(row) => {
                let task = Task::from(row);

                // Log field changes as activity
                if self.config.auto_log_activity {
                    if let Some(ref old) = old_row {
                        self.log_field_changes(id, old, &task).await;
                    }
                }

                // Auto-embed updated task (best-effort)
                if let Some(ref emb) = self.embedding_handler {
                    if let Err(e) = emb.embed_task(&task).await {
                        warn!("Failed to regenerate embedding for task {}: {}", task.id, e);
                    }
                }

                Ok(format!("Updated task: {}", task.title))
            }
            Err(storage::StorageError::NotFound(msg)) => {
                Err(ToolError::ExecutionFailed(format!("Task not found: {}", msg)).into())
            }
            Err(e) => Err(e.into()),
        }
    }

    pub(crate) async fn handle_complete(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;

        // Check for incomplete blockers
        let blocker_rows = self.repo.incomplete_blockers(id).await?;
        if !blocker_rows.is_empty() {
            let blocker_list: Vec<String> = blocker_rows
                .iter()
                .map(|b| format!("[{}] {}", b.id, b.title))
                .collect();
            return Err(ToolError::ExecutionFailed(format!(
                "Cannot complete: blocked by {} incomplete task(s):\n  {}",
                blocker_rows.len(),
                blocker_list.join("\n  ")
            ))
            .into());
        }

        // Close any open time entries
        let te_rows = self.repo.list_time_entries(id).await?;
        for entry in &te_rows {
            if entry.ended_at.is_none() {
                self.repo.close_time_entry(id, entry.id).await?;
            }
        }

        // Capture key_result_id and estimated_minutes before update
        let pre_task = self.repo.get(id).await?;
        let key_result_id = pre_task.as_ref().and_then(|r| r.key_result_id.clone());
        let estimated_minutes = pre_task.as_ref().and_then(|r| r.estimated_minutes);

        // Collect tasks this will unblock
        let blocking_rows = self.repo.get_blocking(id).await?;
        let blocks_ids: Vec<String> = blocking_rows.iter().map(|r| r.id.clone()).collect();

        let patch = TaskPatch {
            id: id.to_string(),
            status: Some("done".to_string()),
            completed: Some(true),
            ..Default::default()
        };

        let result = match self.repo.update(&patch).await {
            Ok(row) => {
                let task = Task::from(row);
                let completed_count = self.repo.cascade_complete(id).await?;

                let mut msg = if completed_count == 0 {
                    format!("Completed: {}", task.title)
                } else {
                    format!(
                        "Completed: {} ({} subtasks also completed)",
                        task.title, completed_count
                    )
                };

                // Report newly unblocked tasks
                if !blocks_ids.is_empty() {
                    let mut unblocked = Vec::new();
                    for bid in &blocks_ids {
                        let remaining = self.repo.incomplete_blockers(bid).await?;
                        if remaining.is_empty() {
                            if let Some(t) = self.repo.get(bid).await? {
                                unblocked.push(format!("[{}] {}", t.id, t.title));
                            }
                        }
                    }
                    if !unblocked.is_empty() {
                        msg.push_str(&format!(
                            "\n\nUnblocked {} task(s):\n  {}",
                            unblocked.len(),
                            unblocked.join("\n  ")
                        ));
                    }
                }

                // Track estimation accuracy
                if self.config.estimation_tracking {
                    if let Some(est_mins) = estimated_minutes {
                        let actual_mins = {
                            let total_secs: i64 =
                                te_rows.iter().filter_map(|e| e.duration_secs).sum();
                            if total_secs > 0 {
                                (total_secs / 60) as i32
                            } else {
                                0
                            }
                        };
                        if actual_mins > 0 {
                            let deviation_pct = if est_mins > 0 {
                                ((actual_mins as f64 - est_mins as f64) / est_mins as f64) * 100.0
                            } else {
                                0.0
                            };
                            let estimation = storage::TaskEstimationRow {
                                id: uuid::Uuid::new_v4().to_string(),
                                task_id: id.to_string(),
                                estimated_minutes: est_mins,
                                actual_minutes: actual_mins,
                                deviation_pct,
                                complexity_score: task.complexity_score,
                                energy_level: task.energy_level.as_ref().map(|e| e.to_string()),
                                tags: task.tags.clone(),
                                completed_at: Utc::now(),
                            };
                            if let Err(e) = self.repo.record_estimation(&estimation).await {
                                warn!("Failed to record estimation history: {}", e);
                            } else if let Some(ref bus) = self.domain_bus {
                                let _ = bus.publish(DomainEvent::EstimationRecorded {
                                    task_id: id.to_string(),
                                    estimated_mins: est_mins as u32,
                                    actual_mins: actual_mins as u32,
                                    deviation_pct,
                                });
                            }
                        }
                    }
                }

                // Log activity
                if self.config.auto_log_activity {
                    let _ = self
                        .repo
                        .log_activity(
                            id,
                            "status_change",
                            Some("status"),
                            Some("todo"),
                            Some("done"),
                            "user",
                            Some("Task completed"),
                        )
                        .await;
                }

                // Emit domain event for task completion
                if let Some(ref bus) = self.domain_bus {
                    let actual_mins = {
                        let total_secs: i64 = te_rows.iter().filter_map(|e| e.duration_secs).sum();
                        if total_secs > 0 {
                            Some(total_secs / 60)
                        } else {
                            None
                        }
                    };
                    let estimated_mins = estimated_minutes.map(|m| m as i64);
                    bus.publish(DomainEvent::TaskCompleted {
                        task_id: task.id.clone(),
                        actual_duration_mins: actual_mins,
                        estimated_duration_mins: estimated_mins,
                        deviation_pct: None,
                    });
                }

                // Cascade progress to KR if task is linked
                if let Some(ref kr_id) = key_result_id {
                    if let Some(ref handler) = self.progress_handler {
                        if let Err(e) = handler.recalculate_kr_progress(kr_id).await {
                            warn!("Failed to cascade progress for KR {}: {}", kr_id, e);
                        }
                    }
                }

                Ok(msg)
            }
            Err(storage::StorageError::NotFound(msg)) => {
                Err(ToolError::ExecutionFailed(format!("Task not found: {}", msg)).into())
            }
            Err(e) => Err(e.into()),
        };

        result
    }

    pub(crate) async fn handle_delete(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;

        if self.repo.delete(id).await? {
            Ok(format!("Deleted task: {}", id))
        } else {
            Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into())
        }
    }

    // ─── Activity logging helper ────────────────────────────────────

    async fn log_field_changes(&self, task_id: &str, old: &storage::TaskRow, new: &Task) {
        let changes: Vec<(&str, String, String)> = {
            let mut v = Vec::new();
            if old.title != new.title {
                v.push(("title", old.title.clone(), new.title.clone()));
            }
            if old.status != new.status {
                v.push(("status", old.status.clone(), new.status.clone()));
            }
            if old.priority != new.priority {
                v.push((
                    "priority",
                    old.priority.map(|p| p.to_string()).unwrap_or_default(),
                    new.priority.map(|p| p.to_string()).unwrap_or_default(),
                ));
            }
            v
        };

        for (field, old_val, new_val) in changes {
            let _ = self
                .repo
                .log_activity(
                    task_id,
                    "field_update",
                    Some(field),
                    Some(&old_val),
                    Some(&new_val),
                    "user",
                    None,
                )
                .await;
        }
    }
}
