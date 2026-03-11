//! Update, complete, enrich, focus, unfocus, move, attach, detach, and log_time handlers.

use chrono::Utc;
use common::{Result, ToolError};
use tools_core::ParamExtractor;
use tracing::warn;

use bus::DomainEvent;

use super::super::TaskTool;
use crate::types::{Action, AttachmentType};
use storage::ActionPatch;

impl TaskTool {
    pub(crate) async fn handle_update(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;

        let patch = ActionPatch {
            id: id.to_string(),
            title: p.optional_str("title")?.map(String::from),
            description: p.optional_str("description")?.map(|s| Some(s.to_string())),
            priority: p.optional_u64("priority")?.map(|v| Some(v as i16)),
            due_date: p.optional_str("due_date")?.map(|s| {
                if s.is_empty() || s == "null" {
                    None
                } else {
                    common::utils::date::parse_datetime(s, &self.timezone)
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
        };

        match self.repo.update(&patch).await {
            Ok(row) => {
                let action = Action::from(row);

                // Auto-embed updated task (best-effort)
                if let Some(ref emb) = self.embedding_handler {
                    if let Err(e) = emb.embed_todo(&action).await {
                        warn!(
                            "Failed to regenerate embedding for action {}: {}",
                            action.id, e
                        );
                    }
                }

                Ok(format!("Updated task: {}", action.title))
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

        // Capture key_result_id before update for progress cascade
        let key_result_id = self
            .repo
            .get(id)
            .await?
            .and_then(|r| r.key_result_id.clone());

        // Collect tasks this will unblock
        let blocking_rows = self.repo.get_blocking(id).await?;
        let blocks_ids: Vec<String> = blocking_rows.iter().map(|r| r.id.clone()).collect();

        let patch = ActionPatch {
            id: id.to_string(),
            status: Some("done".to_string()),
            ..Default::default()
        };

        let result = match self.repo.update(&patch).await {
            Ok(row) => {
                let action = Action::from(row);
                let completed_count = self.repo.cascade_complete(id).await?;

                let mut msg = if completed_count == 0 {
                    format!("Completed: {}", action.title)
                } else {
                    format!(
                        "Completed: {} ({} subtasks also completed)",
                        action.title, completed_count
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

                // Emit domain event for task completion
                if let Some(ref bus) = self.domain_bus {
                    // Reuse te_rows from the close-open-entries step above
                    let actual_mins = {
                        let total_secs: i64 = te_rows.iter().filter_map(|e| e.duration_secs).sum();
                        if total_secs > 0 {
                            Some(total_secs / 60)
                        } else {
                            None
                        }
                    };
                    let estimated_mins = action.estimated_minutes.map(|m| m as i64);
                    bus.publish(DomainEvent::TaskCompleted {
                        task_id: action.id.clone(),
                        actual_duration_mins: actual_mins,
                        estimated_duration_mins: estimated_mins,
                        deviation_pct: None,
                    });
                }

                // Cascade progress to KR if action is linked
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

    pub(crate) async fn handle_enrich(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;

        let task = self
            .get_full_action(id)
            .await?
            .ok_or_else(|| ToolError::ExecutionFailed(format!("Task not found: {}", id)))?;

        let handler = match &self.enrichment_handler {
            Some(h) => h,
            None => {
                return Err(
                    ToolError::ExecutionFailed("Enrichment is not enabled".to_string()).into(),
                )
            }
        };

        let suggestions = handler.enrich_task(&task).await?;

        match suggestions {
            None => Ok(format!(
                "No enrichment suggestions for task {}: all fields already set or enrichment disabled",
                id
            )),
            Some(result) => {
                let mut patch = ActionPatch {
                    id: id.to_string(),
                    ..Default::default()
                };
                let mut applied = Vec::new();
                let mut suggested = Vec::new();

                if let Some(ref priority_sug) = result.priority {
                    suggested.push(format!(
                        "Priority: {} (confidence: {:.0}%) - {}",
                        priority_sug.value,
                        priority_sug.confidence * 100.0,
                        priority_sug.reasoning
                    ));
                    patch.priority = Some(Some(priority_sug.value as i16));
                    applied.push(format!("P{}", priority_sug.value));
                }

                if let Some(ref duration_sug) = result.estimated_minutes {
                    suggested.push(format!(
                        "Duration: {} minutes (confidence: {:.0}%) - {}",
                        duration_sug.value,
                        duration_sug.confidence * 100.0,
                        duration_sug.reasoning
                    ));
                    patch.estimated_minutes = Some(Some(duration_sug.value as i32));
                    applied.push(format!("~{}min", duration_sug.value));
                }

                if let Some(ref due_sug) = result.due_date {
                    let formatted = due_sug.value.format("%Y-%m-%d").to_string();
                    suggested.push(format!(
                        "Due date: {} (confidence: {:.0}%) - {}",
                        formatted,
                        due_sug.confidence * 100.0,
                        due_sug.reasoning
                    ));
                    patch.due_date = Some(Some(due_sug.value));
                    applied.push(format!("due {}", formatted));
                }

                if suggested.is_empty() {
                    return Ok(format!("No enrichment suggestions for task {}", id));
                }

                self.repo.update(&patch).await?;
                self.record_enrichment_feedback(id, &result, Some(true))
                    .await;

                Ok(format!(
                    "Enriched task {} with: {}\n\nSuggestions:\n{}",
                    id,
                    applied.join(", "),
                    suggested.join("\n")
                ))
            }
        }
    }

    pub(crate) async fn handle_focus(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;
        let deadline = Some(Utc::now() + chrono::Duration::hours(self.focus_deadline_hours as i64));

        if self
            .repo
            .focus(id, self.max_focus_slots as i64, deadline)
            .await?
        {
            let te_rows = self.repo.list_time_entries(id).await?;
            let has_running = te_rows.iter().any(|e| e.ended_at.is_none());
            if !has_running {
                self.repo
                    .add_time_entry(id, "focus", Utc::now(), None, None)
                    .await?;
            }
            Ok(format!("Focused on task: {}", id))
        } else {
            Err(ToolError::ExecutionFailed(format!(
                "Cannot focus task {} (not found, already focused, or max slots reached)",
                id
            ))
            .into())
        }
    }

    pub(crate) async fn handle_unfocus(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;

        // Close any open time entries before unfocusing
        let te_rows = self.repo.list_time_entries(id).await?;
        for entry in &te_rows {
            if entry.ended_at.is_none() {
                self.repo.close_time_entry(id, entry.id).await?;
            }
        }

        if self.repo.unfocus(id).await? {
            Ok(format!("Unfocused task: {}", id))
        } else {
            Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into())
        }
    }

    pub(crate) async fn handle_move(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;
        let new_parent_id = p.optional_str("new_parent_id")?.map(String::from);
        let new_project_id = p.optional_str("new_project_id")?.map(String::from);

        if let Some(ref pid) = new_parent_id {
            if self.repo.get(pid).await?.is_none() {
                return Err(ToolError::InvalidParams(format!(
                    "New parent task not found: {}",
                    pid
                ))
                .into());
            }
        }

        let result = match self
            .repo
            .move_action(id, new_parent_id.as_deref(), new_project_id.as_deref())
            .await
        {
            Ok(row) => {
                let action = Action::from(row);
                let mut parts = vec![format!("Moved task: {}", action.title)];
                if new_parent_id.is_some() {
                    parts.push(format!("new parent: {:?}", new_parent_id));
                }
                if new_project_id.is_some() {
                    parts.push(format!("new project: {:?}", new_project_id));
                }
                Ok(parts.join(", "))
            }
            Err(storage::StorageError::NotFound(msg)) => {
                Err(ToolError::ExecutionFailed(format!("Task not found: {}", msg)).into())
            }
            Err(e) => Err(e.into()),
        };

        result
    }

    pub(crate) async fn handle_attach(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;
        let attachment_type_str = p.required_str("attachment_type")?;
        let value = p.required_str("value")?;

        let att_type = AttachmentType::parse(attachment_type_str).ok_or_else(|| {
            ToolError::InvalidParams("attachment_type must be 'file', 'url', or 'note'".to_string())
        })?;

        let title_opt = p.optional_str("attachment_title")?;
        let tags: Vec<String> = Vec::new();

        self.repo
            .add_attachment(id, att_type.as_str(), value, title_opt, &tags)
            .await?;

        Ok(format!(
            "Attachment added to task: {} ({}: {})",
            id, attachment_type_str, value
        ))
    }

    pub(crate) async fn handle_detach(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;
        let attachment_id_str = p.required_str("attachment_id")?;
        let attachment_id = uuid::Uuid::parse_str(attachment_id_str).map_err(|_| {
            ToolError::InvalidParams(format!("Invalid attachment ID: {}", attachment_id_str))
        })?;

        if self.repo.remove_attachment(id, attachment_id).await? {
            Ok(format!(
                "Attachment {} removed from task: {}",
                attachment_id, id
            ))
        } else {
            Err(ToolError::ExecutionFailed("Task or attachment not found".to_string()).into())
        }
    }

    pub(crate) async fn handle_log_time(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;
        let duration_minutes = p.required_u64("duration_minutes")?;
        let duration_secs = (duration_minutes * 60) as i64;
        let now = Utc::now();
        let started_at = now - chrono::Duration::seconds(duration_secs);
        let note = p.optional_str("note")?;

        self.repo
            .add_time_entry(id, "manual", started_at, Some(duration_secs), note)
            .await?;

        Ok(format!(
            "Logged {} minutes to task: {}",
            duration_minutes, id
        ))
    }
}
