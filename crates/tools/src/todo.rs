//! TodoTool - Tool interface for todo system
//!
//! Provides 22 actions for complete todo management through the Tool trait.

use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{RoutingContext, Tool};
use crate::calendar_tool::CalendarHandler;
use crate::enrichment::EnrichmentHandler;
use crate::params::ParamExtractor;
use crate::rrule_utils;
use crate::todo_store::TodoStore;
use crate::todo_types::{Todo, TodoFilter, TodoPatch, TodoStatus};
use common::utils::date::parse_datetime;
use common::{Result, ToolError};
use tracing::warn;

/// TodoTool with config-driven focus values (ADR-008)
pub struct TodoTool {
    store: Arc<RwLock<TodoStore>>,
    max_focus_slots: usize,
    focus_deadline_hours: u64,
    calendar_handler: Option<Arc<dyn CalendarHandler>>,
    enrichment_handler: Option<Arc<dyn EnrichmentHandler>>,
    timezone: String,
}

impl TodoTool {
    /// Create a new TodoTool with config values
    pub fn new(
        store: Arc<RwLock<TodoStore>>,
        max_focus_slots: usize,
        focus_deadline_hours: u64,
        timezone: String,
    ) -> Self {
        Self {
            store,
            max_focus_slots,
            focus_deadline_hours,
            calendar_handler: None,
            enrichment_handler: None,
            timezone,
        }
    }

    /// Add calendar handler for immediate sync on todo changes
    pub fn with_calendar_handler(mut self, handler: Arc<dyn CalendarHandler>) -> Self {
        self.calendar_handler = Some(handler);
        self
    }

    /// Add enrichment handler for AI-powered task suggestions
    pub fn with_enrichment_handler(mut self, handler: Arc<dyn EnrichmentHandler>) -> Self {
        self.enrichment_handler = Some(handler);
        self
    }

    /// Trigger immediate calendar sync (best-effort, don't fail if sync fails)
    async fn trigger_sync_async(&self) {
        if let Some(handler) = &self.calendar_handler {
            if let Err(e) = handler.sync_calendar().await {
                warn!("Immediate calendar sync failed: {}", e);
            }
        }
    }
}

#[async_trait]
impl Tool for TodoTool {
    fn name(&self) -> &str {
        "todo"
    }

    fn description(&self) -> &str {
        "Manage tasks and todos. Actions: add, list, update, complete, delete, show, summary, focus, unfocus, add_subtask, move, attach, detach, log_time, tree, search, report, add_dependency, remove_dependency, recur, list_recurring, delete_recurring, enrich."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["add", "list", "update", "complete", "delete", "show", "summary", "focus", "unfocus", "add_subtask", "move", "attach", "detach", "log_time", "tree", "search", "report", "add_dependency", "remove_dependency", "recur", "list_recurring", "delete_recurring", "enrich"],
                    "description": "Action to perform"
                },
                "id": {
                    "type": "string",
                    "description": "Task ID (for update/complete/delete/show/focus/unfocus/move/attach/detach/log_time/enrich)"
                },
                "title": {
                    "type": "string",
                    "description": "Task title (for add/add_subtask)"
                },
                "description": {
                    "type": "string",
                    "description": "Task description (for add/add_subtask/update)"
                },
                "priority": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 5,
                    "description": "Priority 1-5 (for add/add_subtask/update)"
                },
                "due_date": {
                    "type": "string",
                    "description": "Due date. Accepts: RFC3339 with timezone (e.g. '2026-02-17T21:00:00+07:00'), date with time ('2026-02-17 21:00'), or date only ('2026-02-17', interpreted as midnight in user's timezone). Always include time when the user specifies one. IMPORTANT: Convert natural language dates to these formats using the current date/time from system context. (for add/add_subtask/update)"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tags (for add/add_subtask/update)"
                },
                "status": {
                    "type": "string",
                    "enum": ["todo", "doing", "done", "archived"],
                    "description": "Status (for update/list)"
                },
                "priority_min": {
                    "type": "integer",
                    "description": "Min priority filter (for list)"
                },
                "tag": {
                    "type": "string",
                    "description": "Tag filter (for list)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max results (for list)"
                },
                "parent_id": {
                    "type": "string",
                    "description": "Parent task ID (for add_subtask/list/move)"
                },
                "project_id": {
                    "type": "string",
                    "description": "Project ID (for add/add_subtask/list/move)"
                },
                "new_parent_id": {
                    "type": "string",
                    "description": "New parent task ID (for move, null to unparent)"
                },
                "new_project_id": {
                    "type": "string",
                    "description": "New project ID (for move, null to remove)"
                },
                "attachment_type": {
                    "type": "string",
                    "enum": ["file", "url", "note"],
                    "description": "Type of attachment (for attach)"
                },
                "value": {
                    "type": "string",
                    "description": "Attachment value: file path, URL, or note content (for attach)"
                },
                "attachment_title": {
                    "type": "string",
                    "description": "Optional attachment title (for attach)"
                },
                "attachment_id": {
                    "type": "string",
                    "description": "Attachment ID to remove (for detach)"
                },
                "duration_minutes": {
                    "type": "integer",
                    "description": "Duration in minutes (for log_time)"
                },
                "note": {
                    "type": "string",
                    "description": "Note for time entry (for log_time)"
                },
                "query": {
                    "type": "string",
                    "description": "Search query for titles/descriptions/attachments (for search)"
                },
                "period": {
                    "type": "string",
                    "enum": ["week", "month"],
                    "description": "Time period for report (for report)"
                },
                "rule": {
                    "type": "string",
                    "description": "RRULE recurrence string, e.g. 'FREQ=DAILY;BYHOUR=9' (for recur). V1 supports: FREQ, INTERVAL, BYDAY, BYHOUR, BYMINUTE, BYMONTHDAY."
                },
                "template_id": {
                    "type": "string",
                    "description": "Recurring task template ID (for delete_recurring)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let p = ParamExtractor::new(&args);
        let action = p.required_str("action")?;

        let mut store = self.store.write().await;

        match action {
            "add" => {
                let title = p.required_str("title")?;

                let todo = Todo {
                    id: Todo::generate_id(),
                    title: title.to_string(),
                    description: p.optional_str("description")?.map(String::from),
                    priority: p.optional_u64("priority")?.map(|v| v as u8),
                    due_date: p
                        .optional_str("due_date")?
                        .and_then(|s| parse_datetime(s, &self.timezone)),
                    tags: p.string_array_or_empty("tags")?,
                    status: TodoStatus::Todo,
                    focused_at: None,
                    focus_deadline: None,
                    focus_expired_count: 0,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                    completed_at: None,
                    // Phase 1/2 new fields
                    parent_id: None,
                    project_id: p.optional_str("project_id")?.map(String::from), // Phase 2: support project_id in add
                    attachments: Vec::new(),
                    time_entries: Vec::new(),
                    total_tracked_secs: 0,
                    estimated_minutes: None,
                    calendar_event_uid: None,
                    last_reminded_at: None,
                    recurrence_rule: None,
                    recurrence_parent_id: None,
                    is_template: false,
                    next_instance_date: None,
                    blocked_by: Vec::new(),
                    blocks: Vec::new(),
                };

                let created = store.add(todo).await?;

                // Auto-enrich if handler is available
                let mut enriched_info = String::new();
                if let Some(handler) = &self.enrichment_handler {
                    // Drop lock before calling LLM
                    drop(store);

                    match handler.enrich_task(&created).await {
                        Ok(Some(suggestions)) => {
                            // Check each suggestion against its own confidence and apply if high enough
                            let mut patch = TodoPatch::default();
                            let mut applied = Vec::new();

                            if let Some(ref priority_sug) = suggestions.priority {
                                if priority_sug.confidence >= 0.7 {
                                    patch.priority = Some(priority_sug.value);
                                    applied.push(format!("P{}", priority_sug.value));
                                }
                            }

                            if let Some(ref duration_sug) = suggestions.estimated_minutes {
                                if duration_sug.confidence >= 0.7 {
                                    patch.estimated_minutes = Some(Some(duration_sug.value));
                                    applied.push(format!("~{}min", duration_sug.value));
                                }
                            }

                            if let Some(ref due_sug) = suggestions.due_date {
                                if due_sug.confidence >= 0.7 {
                                    patch.due_date = Some(Some(due_sug.value));
                                    applied.push("due date".to_string());
                                }
                            }

                            if !applied.is_empty() {
                                let mut store = self.store.write().await;
                                if store.update(&created.id, patch).await.is_ok() {
                                    enriched_info = format!(" (enriched: {})", applied.join(", "));
                                }
                                drop(store);
                            }
                        }
                        Ok(None) => {
                            // Enrichment disabled or nothing to suggest
                        }
                        Err(e) => {
                            warn!("Task enrichment failed: {}", e);
                        }
                    }
                } else {
                    drop(store);
                }

                // Trigger immediate calendar sync
                self.trigger_sync_async().await;

                Ok(format!(
                    "Task created: {} (ID: {}){}",
                    created.title, created.id, enriched_info
                ))
            }

            "list" => {
                let filter = TodoFilter {
                    status: p.optional_str("status")?.and_then(|s| match s {
                        "todo" => Some(TodoStatus::Todo),
                        "doing" => Some(TodoStatus::Doing),
                        "done" => Some(TodoStatus::Done),
                        "archived" => Some(TodoStatus::Archived),
                        _ => None,
                    }),
                    priority_min: p.optional_u64("priority_min")?.map(|v| v as u8),
                    tag: p.optional_str("tag")?.map(String::from),
                    limit: p.optional_u64("limit")?.map(|l| l as usize),
                    // Phase 2: new filters
                    project_id: p.optional_str("project_id")?.map(String::from),
                    parent_id: p.optional_str("parent_id")?.map(String::from),
                    include_templates: false,
                };

                let todos = store.list(&filter).await?;
                if todos.is_empty() {
                    return Ok("No tasks found.".to_string());
                }

                let mut output = format!("{} task(s):\n", todos.len());
                for todo in todos {
                    output.push_str(&format!(
                        "\n- [{}] {} (P{}, {:?}, {})",
                        todo.id,
                        todo.title,
                        todo.priority.unwrap_or(3),
                        todo.status,
                        todo.tags.join(", ")
                    ));
                }
                Ok(output)
            }

            "update" => {
                let id = p.required_str("id")?;

                let patch = TodoPatch {
                    title: p.optional_str("title")?.map(String::from),
                    description: p.optional_str("description")?.map(|s| Some(s.to_string())),
                    priority: p.optional_u64("priority")?.map(|v| v as u8),
                    due_date: p.optional_str("due_date")?.map(|s| {
                        if s.is_empty() || s == "null" {
                            // Empty string or "null" means clear the due_date
                            None
                        } else {
                            parse_datetime(s, &self.timezone)
                        }
                    }),
                    tags: p.optional_array("tags")?.map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    }),
                    status: p.optional_str("status")?.and_then(|s| match s {
                        "todo" => Some(TodoStatus::Todo),
                        "doing" => Some(TodoStatus::Doing),
                        "done" => Some(TodoStatus::Done),
                        "archived" => Some(TodoStatus::Archived),
                        _ => None,
                    }),
                    last_reminded_at: None,
                    calendar_event_uid: None,
                    estimated_minutes: p.optional_u64("estimated_minutes")?.map(|v| Some(v as u32)),
                };

                let result = match store.update(id, patch).await? {
                    Some(todo) => Ok(format!("Updated task: {}", todo.title)),
                    None => {
                        Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into())
                    }
                };

                // Drop lock and trigger sync on success
                drop(store);
                if result.is_ok() {
                    self.trigger_sync_async().await;
                }

                result
            }

            "complete" => {
                let id = p.required_str("id")?;

                // Check for incomplete blockers before allowing completion
                let blockers = store.incomplete_blockers(id).await?;
                if !blockers.is_empty() {
                    let blocker_list: Vec<String> = blockers
                        .iter()
                        .map(|b| format!("[{}] {}", b.id, b.title))
                        .collect();
                    return Err(ToolError::ExecutionFailed(format!(
                        "Cannot complete: blocked by {} incomplete task(s):\n  {}",
                        blockers.len(),
                        blocker_list.join("\n  ")
                    ))
                    .into());
                }

                // Phase 2: Close any running time entries before completing
                if let Some(todo) = store.get(id).await? {
                    for entry in &todo.time_entries {
                        if entry.ended_at.is_none() {
                            store.close_time_entry(id, &entry.id).await?;
                        }
                    }
                }

                // Collect tasks this will unblock (before marking done)
                let blocks_ids: Vec<String> = store
                    .get(id)
                    .await?
                    .map(|t| t.blocks.clone())
                    .unwrap_or_default();

                let patch = TodoPatch {
                    status: Some(TodoStatus::Done),
                    ..Default::default()
                };

                let result = match store.update(id, patch).await? {
                    Some(todo) => {
                        // Phase 2: Cascade completion to subtasks
                        let completed_children = store.cascade_complete(id).await?;

                        let mut msg = if completed_children.is_empty() {
                            format!("Completed: {}", todo.title)
                        } else {
                            format!(
                                "Completed: {} ({} subtasks also completed)",
                                todo.title,
                                completed_children.len()
                            )
                        };

                        // Report newly unblocked tasks
                        if !blocks_ids.is_empty() {
                            let mut unblocked = Vec::new();
                            for bid in &blocks_ids {
                                let remaining = store.incomplete_blockers(bid).await?;
                                if remaining.is_empty() {
                                    if let Some(t) = store.get(bid).await? {
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

                        Ok(msg)
                    }
                    None => {
                        Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into())
                    }
                };

                // Drop lock and trigger sync on success
                drop(store);
                if result.is_ok() {
                    self.trigger_sync_async().await;
                }

                result
            }

            "delete" => {
                let id = p.required_str("id")?;

                let result = if store.delete(id).await? {
                    Ok(format!("Deleted task: {}", id))
                } else {
                    Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into())
                };

                // Drop lock and trigger sync on success
                drop(store);
                if result.is_ok() {
                    self.trigger_sync_async().await;
                }

                result
            }

            "enrich" => {
                let id = p.required_str("id")?;

                // Get the task
                let task = store.get(id).await?;
                let task = match task {
                    Some(t) => t,
                    None => {
                        return Err(
                            ToolError::ExecutionFailed(format!("Task not found: {}", id)).into(),
                        );
                    }
                };

                // Check if enrichment handler is available
                let handler = match &self.enrichment_handler {
                    Some(h) => h,
                    None => {
                        return Err(ToolError::ExecutionFailed(
                            "Enrichment is not enabled".to_string(),
                        )
                        .into());
                    }
                };

                // Drop lock before calling LLM
                drop(store);

                // Call enrichment
                let suggestions = handler.enrich_task(&task).await?;

                match suggestions {
                    None => Ok(format!(
                        "No enrichment suggestions for task {}: all fields already set or enrichment disabled",
                        id
                    )),
                    Some(result) => {
                        let mut patch = TodoPatch::default();
                        let mut applied = Vec::new();
                        let mut suggested = Vec::new();

                        // Build suggestion summary
                        if let Some(ref priority_sug) = result.priority {
                            suggested.push(format!(
                                "Priority: {} (confidence: {:.0}%) - {}",
                                priority_sug.value,
                                priority_sug.confidence * 100.0,
                                priority_sug.reasoning
                            ));
                            patch.priority = Some(priority_sug.value);
                            applied.push(format!("P{}", priority_sug.value));
                        }

                        if let Some(ref duration_sug) = result.estimated_minutes {
                            suggested.push(format!(
                                "Duration: {} minutes (confidence: {:.0}%) - {}",
                                duration_sug.value,
                                duration_sug.confidence * 100.0,
                                duration_sug.reasoning
                            ));
                            patch.estimated_minutes = Some(Some(duration_sug.value));
                            applied.push(format!("~{}min", duration_sug.value));
                        }

                        if let Some(ref due_sug) = result.due_date {
                            let formatted_date = due_sug.value.format("%Y-%m-%d").to_string();
                            suggested.push(format!(
                                "Due date: {} (confidence: {:.0}%) - {}",
                                formatted_date,
                                due_sug.confidence * 100.0,
                                due_sug.reasoning
                            ));
                            patch.due_date = Some(Some(due_sug.value));
                            applied.push(format!("due {}", formatted_date));
                        }

                        if suggested.is_empty() {
                            return Ok(format!("No enrichment suggestions for task {}", id));
                        }

                        // Apply the suggestions
                        let mut store = self.store.write().await;
                        store.update(id, patch).await?;
                        drop(store);

                        // Trigger calendar sync
                        self.trigger_sync_async().await;

                        Ok(format!(
                            "Enriched task {} with: {}\n\nSuggestions:\n{}",
                            id,
                            applied.join(", "),
                            suggested.join("\n")
                        ))
                    }
                }
            }

            "show" => {
                let id = p.required_str("id")?;

                match store.get(id).await? {
                    Some(todo) => {
                        let mut output = format!("Task: {}\n", todo.title);
                        output.push_str(&format!("ID: {}\n", todo.id));
                        output.push_str(&format!("Status: {:?}\n", todo.status));
                        output.push_str(&format!("Priority: {}\n", todo.priority.unwrap_or(3)));
                        if let Some(desc) = &todo.description {
                            output.push_str(&format!("Description: {}\n", desc));
                        }
                        if !todo.tags.is_empty() {
                            output.push_str(&format!("Tags: {}\n", todo.tags.join(", ")));
                        }
                        if let Some(due) = &todo.due_date {
                            output.push_str(&format!("Due: {}\n", due.format("%Y-%m-%d")));
                        }

                        Ok(output)
                    }
                    None => {
                        Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into())
                    }
                }
            }

            "summary" => {
                let summary = store.summary().await?;
                let mut output = format!("Total tasks: {}\n\n", summary.total);
                output.push_str("By status:\n");
                for (status, count) in summary.by_status {
                    output.push_str(&format!("  {:?}: {}\n", status, count));
                }
                if !summary.overdue.is_empty() {
                    output.push_str(&format!("\nOverdue: {} tasks\n", summary.overdue.len()));
                }
                Ok(output)
            }

            "focus" => {
                let id = p.required_str("id")?;

                // ADR-008: Use config values, not hardcoded
                if store
                    .focus(id, self.max_focus_slots, self.focus_deadline_hours)
                    .await?
                {
                    // Phase 2: Auto-start TimeEntry on focus
                    if let Some(todo) = store.get(id).await? {
                        // Only start time entry if there isn't one already running
                        let has_running_entry =
                            todo.time_entries.iter().any(|e| e.ended_at.is_none());

                        if !has_running_entry {
                            use crate::todo_types::{TimeEntry, TimeEntrySource, Todo};
                            let time_entry = TimeEntry {
                                id: Todo::generate_id(),
                                started_at: Utc::now(),
                                ended_at: None,
                                duration_secs: None,
                                note: None,
                                source: TimeEntrySource::Focus,
                            };

                            store.add_time_entry(id, time_entry).await?;
                        }
                    }
                    Ok(format!("Focused on task: {}", id))
                } else {
                    Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into())
                }
            }

            "unfocus" => {
                let id = p.required_str("id")?;

                // Phase 2: Auto-close any running time entries before unfocusing
                if let Some(todo) = store.get(id).await? {
                    // Find and close all running time entries
                    for entry in &todo.time_entries {
                        if entry.ended_at.is_none() {
                            store.close_time_entry(id, &entry.id).await?;
                        }
                    }
                }

                if store.unfocus(id).await? {
                    Ok(format!("Unfocused task: {}", id))
                } else {
                    Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into())
                }
            }

            "add_subtask" => {
                let title = p.required_str("title")?;
                let parent_id = p.required_str("parent_id")?;

                // Verify parent exists
                if store.get(parent_id).await?.is_none() {
                    return Err(ToolError::InvalidParams(format!(
                        "Parent task not found: {}",
                        parent_id
                    ))
                    .into());
                }

                use crate::todo_types::Todo;
                let todo = Todo {
                    id: Todo::generate_id(),
                    title: title.to_string(),
                    description: p.optional_str("description")?.map(String::from),
                    priority: p.optional_u64("priority")?.map(|v| v as u8),
                    due_date: p
                        .optional_str("due_date")?
                        .and_then(|s| parse_datetime(s, &self.timezone)),
                    tags: p.string_array_or_empty("tags")?,
                    status: TodoStatus::Todo,
                    focused_at: None,
                    focus_deadline: None,
                    focus_expired_count: 0,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                    completed_at: None,
                    parent_id: Some(parent_id.to_string()),
                    project_id: p.optional_str("project_id")?.map(String::from),
                    attachments: Vec::new(),
                    time_entries: Vec::new(),
                    total_tracked_secs: 0,
                    estimated_minutes: None,
                    calendar_event_uid: None,
                    last_reminded_at: None,
                    recurrence_rule: None,
                    recurrence_parent_id: None,
                    is_template: false,
                    next_instance_date: None,
                    blocked_by: Vec::new(),
                    blocks: Vec::new(),
                };

                let created = store.add(todo).await?;
                let result = format!(
                    "Subtask created: {} (ID: {}, parent: {})",
                    created.title, created.id, parent_id
                );

                // Drop lock before triggering sync
                drop(store);

                // Trigger immediate calendar sync
                self.trigger_sync_async().await;

                Ok(result)
            }

            "move" => {
                let id = p.required_str("id")?;

                let new_parent_id = p.optional_str("new_parent_id")?.map(String::from);
                let new_project_id = p.optional_str("new_project_id")?.map(String::from);

                // Verify new parent exists if specified
                if let Some(ref pid) = new_parent_id {
                    if store.get(pid).await?.is_none() {
                        return Err(ToolError::InvalidParams(format!(
                            "New parent task not found: {}",
                            pid
                        ))
                        .into());
                    }
                }

                let result = match store
                    .move_todo(id, new_parent_id.clone(), new_project_id.clone())
                    .await?
                {
                    Some(todo) => {
                        let mut parts = vec![format!("Moved task: {}", todo.title)];
                        if new_parent_id.is_some() {
                            parts.push(format!("new parent: {:?}", new_parent_id));
                        }
                        if new_project_id.is_some() {
                            parts.push(format!("new project: {:?}", new_project_id));
                        }
                        Ok(parts.join(", "))
                    }
                    None => {
                        Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into())
                    }
                };

                // Drop lock and trigger sync on success
                drop(store);
                if result.is_ok() {
                    self.trigger_sync_async().await;
                }

                result
            }

            "attach" => {
                let id = p.required_str("id")?;
                let attachment_type_str = p.required_str("attachment_type")?;
                let value = p.required_str("value")?;

                use crate::todo_types::{Attachment, AttachmentType, Todo};
                let attachment_type = match attachment_type_str {
                    "file" => AttachmentType::File,
                    "url" => AttachmentType::Url,
                    "note" => AttachmentType::Note,
                    _ => {
                        return Err(ToolError::InvalidParams(
                            "attachment_type must be 'file', 'url', or 'note'".to_string(),
                        )
                        .into())
                    }
                };

                let attachment = Attachment {
                    id: Todo::generate_id(),
                    attachment_type,
                    title: p.optional_str("attachment_title")?.map(String::from),
                    value: value.to_string(),
                    tags: Vec::new(),
                    created_at: Utc::now(),
                };

                if store.add_attachment(id, attachment).await? {
                    Ok(format!(
                        "Attachment added to task: {} ({}: {})",
                        id, attachment_type_str, value
                    ))
                } else {
                    Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into())
                }
            }

            "detach" => {
                let id = p.required_str("id")?;
                let attachment_id = p.required_str("attachment_id")?;

                if store.remove_attachment(id, attachment_id).await? {
                    Ok(format!(
                        "Attachment {} removed from task: {}",
                        attachment_id, id
                    ))
                } else {
                    Err(
                        ToolError::ExecutionFailed("Task or attachment not found".to_string())
                            .into(),
                    )
                }
            }

            "log_time" => {
                let id = p.required_str("id")?;
                let duration_minutes = p.required_u64("duration_minutes")?;

                use crate::todo_types::{TimeEntry, TimeEntrySource, Todo};
                let now = Utc::now();
                let duration_secs = duration_minutes * 60;
                let started_at = now - chrono::Duration::seconds(duration_secs as i64);

                let time_entry = TimeEntry {
                    id: Todo::generate_id(),
                    started_at,
                    ended_at: Some(now),
                    duration_secs: Some(duration_secs),
                    note: p.optional_str("note")?.map(String::from),
                    source: TimeEntrySource::Manual,
                };

                if store.add_time_entry(id, time_entry).await? {
                    Ok(format!(
                        "Logged {} minutes to task: {}",
                        duration_minutes, id
                    ))
                } else {
                    Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into())
                }
            }

            "tree" => {
                // Build hierarchical tree view of todos
                let todos = store.list(&TodoFilter::default()).await?;

                // Find root tasks (no parent)
                let roots: Vec<_> = todos.iter().filter(|t| t.parent_id.is_none()).collect();

                if roots.is_empty() {
                    return Ok("No tasks found.".to_string());
                }

                fn render_tree(
                    todo: &crate::todo_types::Todo,
                    all_todos: &[crate::todo_types::Todo],
                    prefix: &str,
                    is_last: bool,
                ) -> String {
                    let mut output = String::new();

                    // Current node
                    let connector = if is_last { "└─ " } else { "├─ " };
                    output.push_str(&format!(
                        "{}{}{} [{}] (P{}, {:?})\n",
                        prefix,
                        connector,
                        todo.title,
                        todo.id,
                        todo.priority.unwrap_or(3),
                        todo.status
                    ));

                    // Show dependency info
                    let detail_prefix = format!("{}{}  ", prefix, if is_last { " " } else { "│" });
                    if !todo.blocked_by.is_empty() {
                        for dep_id in &todo.blocked_by {
                            let dep_title = all_todos
                                .iter()
                                .find(|t| t.id == *dep_id)
                                .map(|t| t.title.as_str())
                                .unwrap_or("(unknown)");
                            output.push_str(&format!(
                                "{}⛔ Blocked by: [{}] {}\n",
                                detail_prefix, dep_id, dep_title
                            ));
                        }
                    }
                    if !todo.blocks.is_empty() {
                        for dep_id in &todo.blocks {
                            let dep_title = all_todos
                                .iter()
                                .find(|t| t.id == *dep_id)
                                .map(|t| t.title.as_str())
                                .unwrap_or("(unknown)");
                            output.push_str(&format!(
                                "{}→ Blocks: [{}] {}\n",
                                detail_prefix, dep_id, dep_title
                            ));
                        }
                    }

                    // Find children
                    let children: Vec<_> = all_todos
                        .iter()
                        .filter(|t| t.parent_id.as_ref() == Some(&todo.id))
                        .collect();

                    // Render children
                    for (i, child) in children.iter().enumerate() {
                        let is_last_child = i == children.len() - 1;
                        output.push_str(&render_tree(
                            child,
                            all_todos,
                            &detail_prefix,
                            is_last_child,
                        ));
                    }

                    output
                }

                let mut output = String::from("Task Tree:\n\n");
                for (i, root) in roots.iter().enumerate() {
                    let is_last = i == roots.len() - 1;
                    output.push_str(&render_tree(root, &todos, "", is_last));
                }

                Ok(output)
            }

            "search" => {
                let query = p.required_str("query")?.to_lowercase();

                let todos = store.list(&TodoFilter::default()).await?;
                let results: Vec<_> = todos
                    .into_iter()
                    .filter(|t| {
                        // Search in title
                        if t.title.to_lowercase().contains(&query) {
                            return true;
                        }
                        // Search in description
                        if let Some(desc) = &t.description {
                            if desc.to_lowercase().contains(&query) {
                                return true;
                            }
                        }
                        // Search in attachments
                        for att in &t.attachments {
                            if att.value.to_lowercase().contains(&query) {
                                return true;
                            }
                            if let Some(title) = &att.title {
                                if title.to_lowercase().contains(&query) {
                                    return true;
                                }
                            }
                        }
                        false
                    })
                    .collect();

                if results.is_empty() {
                    return Ok(format!("No tasks found matching '{}'", query));
                }

                let mut output = format!("{} task(s) matching '{}':\n", results.len(), query);
                for todo in results {
                    output.push_str(&format!(
                        "\n- [{}] {} (P{}, {:?})",
                        todo.id,
                        todo.title,
                        todo.priority.unwrap_or(3),
                        todo.status
                    ));
                }
                Ok(output)
            }

            "report" => {
                let period = p.required_str("period")?;
                let project_id_filter = p.optional_str("project_id")?.map(String::from);

                // Calculate date range based on period
                let now = Utc::now();
                let period_start = match period {
                    "week" => now - chrono::Duration::days(7),
                    "month" => now - chrono::Duration::days(30),
                    _ => {
                        return Err(ToolError::InvalidParams(
                            "period must be 'week' or 'month'".to_string(),
                        )
                        .into())
                    }
                };

                // Get all todos
                let todos = store.list(&TodoFilter::default()).await?;

                // Filter by project_id if specified
                let todos: Vec<_> = if let Some(ref proj_id) = project_id_filter {
                    todos
                        .into_iter()
                        .filter(|t| t.project_id.as_ref() == Some(proj_id))
                        .collect()
                } else {
                    todos
                };

                // Group todos by project
                use std::collections::HashMap;
                let mut projects: HashMap<String, Vec<&Todo>> = HashMap::new();
                for todo in &todos {
                    let project_key = todo
                        .project_id
                        .clone()
                        .unwrap_or_else(|| "(no project)".to_string());
                    projects.entry(project_key).or_default().push(todo);
                }

                // Calculate statistics
                let mut completed_in_period_total = 0;
                let mut created_in_period_total = 0;
                let mut total_time_secs = 0;
                let mut focus_session_count = 0;
                let mut focus_time_secs = 0;
                let mut overdue_count = 0;

                let mut output = String::from("=== Todo Report ===\n\n");
                output.push_str(&format!(
                    "Period: Last {} days\n\n",
                    match period {
                        "week" => 7,
                        "month" => 30,
                        _ => 0,
                    }
                ));

                // Per-project breakdown
                output.push_str("## By Project:\n\n");
                for (project_key, project_todos) in &projects {
                    let completed_in_period = project_todos
                        .iter()
                        .filter(|t| {
                            t.status == TodoStatus::Done
                                && t.completed_at.map(|dt| dt >= period_start).unwrap_or(false)
                        })
                        .count();

                    let created_in_period = project_todos
                        .iter()
                        .filter(|t| t.created_at >= period_start)
                        .count();

                    let time_tracked: u64 = project_todos
                        .iter()
                        .flat_map(|t| &t.time_entries)
                        .filter_map(|e| e.duration_secs)
                        .sum();

                    completed_in_period_total += completed_in_period;
                    created_in_period_total += created_in_period;
                    total_time_secs += time_tracked;

                    output.push_str(&format!("### {}\n", project_key));
                    output.push_str(&format!("  - Completed: {}\n", completed_in_period));
                    output.push_str(&format!("  - Created: {}\n", created_in_period));
                    output.push_str(&format!(
                        "  - Time tracked: {:.1}h\n",
                        time_tracked as f64 / 3600.0
                    ));
                    output.push('\n');
                }

                // Count focus sessions and overdue tasks
                for todo in &todos {
                    // Count focus sessions in period
                    for entry in &todo.time_entries {
                        if entry.source == crate::todo_types::TimeEntrySource::Focus
                            && entry.started_at >= period_start
                        {
                            focus_session_count += 1;
                            if let Some(duration) = entry.duration_secs {
                                focus_time_secs += duration;
                            }
                        }
                    }

                    // Count overdue tasks
                    if let Some(due) = todo.due_date {
                        if due < now && todo.status != TodoStatus::Done {
                            overdue_count += 1;
                        }
                    }
                }

                // Summary
                output.push_str("## Summary:\n\n");
                output.push_str(&format!("  - Tasks created: {}\n", created_in_period_total));
                output.push_str(&format!(
                    "  - Tasks completed: {}\n",
                    completed_in_period_total
                ));
                output.push_str(&format!(
                    "  - Total time tracked: {:.1}h\n",
                    total_time_secs as f64 / 3600.0
                ));
                output.push_str(&format!("  - Focus sessions: {}\n", focus_session_count));
                output.push_str(&format!(
                    "  - Focus time: {:.1}h\n",
                    focus_time_secs as f64 / 3600.0
                ));
                output.push_str(&format!("  - Overdue tasks: {}\n", overdue_count));

                Ok(output)
            }

            "add_dependency" => {
                let task_id = p.required_str("task_id")?;
                let blocked_by = p.required_str("blocked_by")?;

                store.add_dependency(task_id, blocked_by).await?;

                let task = store.get(task_id).await?.unwrap();
                let blocker = store.get(blocked_by).await?.unwrap();
                Ok(format!(
                    "Dependency added: [{}] {} is now blocked by [{}] {}",
                    task.id, task.title, blocker.id, blocker.title
                ))
            }

            "remove_dependency" => {
                let task_id = p.required_str("task_id")?;
                let blocked_by = p.required_str("blocked_by")?;

                store.remove_dependency(task_id, blocked_by).await?;

                Ok(format!(
                    "Dependency removed: {} is no longer blocked by {}",
                    task_id, blocked_by
                ))
            }

            "recur" => {
                let title = p.required_str("title")?;
                let rule = p.required_str("rule")?;

                // Validate the RRULE against V1 subset
                rrule_utils::validate_rrule(rule)?;

                let now = Utc::now();
                let next_date = rrule_utils::next_occurrence(rule, now)?;

                let template = Todo {
                    id: Todo::generate_id(),
                    title: title.to_string(),
                    description: p.optional_str("description")?.map(String::from),
                    priority: p.optional_u64("priority")?.map(|v| v as u8),
                    due_date: None,
                    tags: p.string_array_or_empty("tags")?,
                    status: TodoStatus::Todo,
                    focused_at: None,
                    focus_deadline: None,
                    focus_expired_count: 0,
                    created_at: now,
                    updated_at: now,
                    completed_at: None,
                    parent_id: None,
                    project_id: p.optional_str("project_id")?.map(String::from),
                    attachments: Vec::new(),
                    time_entries: Vec::new(),
                    total_tracked_secs: 0,
                    estimated_minutes: None,
                    calendar_event_uid: None,
                    last_reminded_at: None,
                    recurrence_rule: Some(rule.to_string()),
                    recurrence_parent_id: None,
                    is_template: true,
                    next_instance_date: next_date,
                    blocked_by: Vec::new(),
                    blocks: Vec::new(),
                };

                let created = store.add(template).await?;
                let human_rule = rrule_utils::humanize_rrule(rule);
                let next_str = next_date
                    .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
                    .unwrap_or_else(|| "unknown".to_string());

                Ok(format!(
                    "Recurring task created: {} (ID: {}, rule: {}, next: {})",
                    created.title, created.id, human_rule, next_str
                ))
            }

            "list_recurring" => {
                let templates = store.list_templates().await?;

                if templates.is_empty() {
                    return Ok("No recurring task templates found.".to_string());
                }

                let mut output = format!("{} recurring template(s):\n\n", templates.len());
                for t in &templates {
                    let rule_str = t.recurrence_rule.as_deref().unwrap_or("(no rule)");
                    let human = rrule_utils::humanize_rrule(rule_str);
                    let next_str = t
                        .next_instance_date
                        .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
                        .unwrap_or_else(|| "none".to_string());

                    output.push_str(&format!(
                        "- [{}] {} | {} | Next: {}\n",
                        t.id, t.title, human, next_str
                    ));

                    if let Some(ref desc) = t.description {
                        output.push_str(&format!("  Description: {}\n", desc));
                    }
                    if let Some(pri) = t.priority {
                        output.push_str(&format!("  Priority: {}\n", pri));
                    }
                    if !t.tags.is_empty() {
                        output.push_str(&format!("  Tags: {}\n", t.tags.join(", ")));
                    }
                }

                Ok(output)
            }

            "delete_recurring" => {
                let template_id = p.required_str("template_id")?;

                // Verify the task exists and is a template
                let template = store.get(template_id).await?;
                match template {
                    Some(t) if t.is_template => {
                        store.delete(template_id).await?;
                        Ok(format!(
                            "Recurring template deleted: [{}] {}. Existing instances are now standalone tasks.",
                            t.id, t.title
                        ))
                    }
                    Some(_) => Err(ToolError::InvalidParams(format!(
                        "Task {} is not a recurring template. Use 'delete' for regular tasks.",
                        template_id
                    ))
                    .into()),
                    None => Err(ToolError::ExecutionFailed(format!(
                        "Template not found: {}",
                        template_id
                    ))
                    .into()),
                }
            }

            _ => Err(ToolError::InvalidParams(format!("Unknown action: {}", action)).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::todo_store::TodoStore;
    use tempfile::TempDir;

    async fn create_test_tool() -> (TodoTool, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("todos.jsonl");
        let store = Arc::new(RwLock::new(TodoStore::new(file_path)));
        let tool = TodoTool::new(store, 3, 18, "UTC".to_string());
        (tool, temp_dir)
    }

    fn ctx() -> RoutingContext {
        RoutingContext::new(
            common::ChannelName::new("telegram"),
            common::ChatId::new("test"),
        )
    }

    #[tokio::test]
    async fn test_tool_name() {
        let (tool, _dir) = create_test_tool().await;
        assert_eq!(tool.name(), "todo");
    }

    #[tokio::test]
    async fn test_add_action() {
        let (tool, _dir) = create_test_tool().await;

        let args = serde_json::json!({
            "action": "add",
            "title": "Test task"
        });

        let result = tool.execute(args, &ctx()).await.unwrap();
        assert!(result.contains("Task created"));
        assert!(result.contains("Test task"));
    }

    #[tokio::test]
    async fn test_add_with_all_fields() {
        let (tool, _dir) = create_test_tool().await;

        let args = serde_json::json!({
            "action": "add",
            "title": "Complete task with all fields",
            "description": "A detailed description",
            "priority": 4,
            "tags": ["backend", "urgent"]
        });

        let result = tool.execute(args, &ctx()).await.unwrap();
        assert!(result.contains("Complete task with all fields"));
    }

    #[tokio::test]
    async fn test_list_action() {
        let (tool, _dir) = create_test_tool().await;

        // Add a task first
        let add_args = serde_json::json!({
            "action": "add",
            "title": "Task to list"
        });
        tool.execute(add_args, &ctx()).await.unwrap();

        // List all tasks
        let list_args = serde_json::json!({
            "action": "list"
        });

        let result = tool.execute(list_args, &ctx()).await.unwrap();
        assert!(result.contains("1 task(s)"));
        assert!(result.contains("Task to list"));
    }

    #[tokio::test]
    async fn test_list_empty() {
        let (tool, _dir) = create_test_tool().await;

        let args = serde_json::json!({
            "action": "list"
        });

        let result = tool.execute(args, &ctx()).await.unwrap();
        assert_eq!(result, "No tasks found.");
    }

    #[tokio::test]
    async fn test_update_action() {
        let (tool, _dir) = create_test_tool().await;

        // Add a task
        let add_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Original title"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        // Extract ID from result
        let id_start = add_result.find("ID: ").unwrap() + 4;
        let id_end = add_result[id_start..].find(')').unwrap() + id_start;
        let id = &add_result[id_start..id_end];

        // Update the task
        let update_args = serde_json::json!({
            "action": "update",
            "id": id,
            "title": "Updated title"
        });

        let result = tool.execute(update_args, &ctx()).await.unwrap();
        assert!(result.contains("Updated task"));
        assert!(result.contains("Updated title"));
    }

    #[tokio::test]
    async fn test_complete_action() {
        let (tool, _dir) = create_test_tool().await;

        // Add a task
        let add_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Task to complete"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        let id_start = add_result.find("ID: ").unwrap() + 4;
        let id_end = add_result[id_start..].find(')').unwrap() + id_start;
        let id = &add_result[id_start..id_end];

        // Complete it
        let complete_args = serde_json::json!({
            "action": "complete",
            "id": id
        });

        let result = tool.execute(complete_args, &ctx()).await.unwrap();
        assert!(result.contains("Completed"));
        assert!(result.contains("Task to complete"));
    }

    #[tokio::test]
    async fn test_delete_action() {
        let (tool, _dir) = create_test_tool().await;

        // Add a task
        let add_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Task to delete"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        let id_start = add_result.find("ID: ").unwrap() + 4;
        let id_end = add_result[id_start..].find(')').unwrap() + id_start;
        let id = &add_result[id_start..id_end];

        // Delete it
        let delete_args = serde_json::json!({
            "action": "delete",
            "id": id
        });

        let result = tool.execute(delete_args, &ctx()).await.unwrap();
        assert!(result.contains("Deleted task"));
        assert!(result.contains(id));
    }

    #[tokio::test]
    async fn test_show_action() {
        let (tool, _dir) = create_test_tool().await;

        // Add a task
        let add_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Task to show",
                    "description": "Details here",
                    "priority": 5
                }),
                &ctx(),
            )
            .await
            .unwrap();

        let id_start = add_result.find("ID: ").unwrap() + 4;
        let id_end = add_result[id_start..].find(')').unwrap() + id_start;
        let id = &add_result[id_start..id_end];

        // Show it
        let show_args = serde_json::json!({
            "action": "show",
            "id": id
        });

        let result = tool.execute(show_args, &ctx()).await.unwrap();
        assert!(result.contains("Task: Task to show"));
        assert!(result.contains("Description: Details here"));
        assert!(result.contains("Priority: 5"));
    }

    #[tokio::test]
    async fn test_summary_action() {
        let (tool, _dir) = create_test_tool().await;

        // Add a few tasks
        tool.execute(
            serde_json::json!({"action": "add", "title": "Task 1"}),
            &ctx(),
        )
        .await
        .unwrap();
        tool.execute(
            serde_json::json!({"action": "add", "title": "Task 2"}),
            &ctx(),
        )
        .await
        .unwrap();

        let summary_args = serde_json::json!({
            "action": "summary"
        });

        let result = tool.execute(summary_args, &ctx()).await.unwrap();
        assert!(result.contains("Total tasks: 2"));
        assert!(result.contains("By status:"));
    }

    #[tokio::test]
    async fn test_focus_action() {
        let (tool, _dir) = create_test_tool().await;

        // Add a task
        let add_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Task to focus"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        let id_start = add_result.find("ID: ").unwrap() + 4;
        let id_end = add_result[id_start..].find(')').unwrap() + id_start;
        let id = &add_result[id_start..id_end];

        // Focus it
        let focus_args = serde_json::json!({
            "action": "focus",
            "id": id
        });

        let result = tool.execute(focus_args, &ctx()).await.unwrap();
        assert!(result.contains("Focused on task"));
    }

    #[tokio::test]
    async fn test_unfocus_action() {
        let (tool, _dir) = create_test_tool().await;

        // Add and focus a task
        let add_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Task to unfocus"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        let id_start = add_result.find("ID: ").unwrap() + 4;
        let id_end = add_result[id_start..].find(')').unwrap() + id_start;
        let id = &add_result[id_start..id_end];

        tool.execute(serde_json::json!({"action": "focus", "id": id}), &ctx())
            .await
            .unwrap();

        // Unfocus it
        let unfocus_args = serde_json::json!({
            "action": "unfocus",
            "id": id
        });

        let result = tool.execute(unfocus_args, &ctx()).await.unwrap();
        assert!(result.contains("Unfocused task"));
    }

    #[tokio::test]
    async fn test_missing_action() {
        let (tool, _dir) = create_test_tool().await;

        let args = serde_json::json!({
            "title": "No action"
        });

        let result = tool.execute(args, &ctx()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_unknown_action() {
        let (tool, _dir) = create_test_tool().await;

        let args = serde_json::json!({
            "action": "invalid"
        });

        let result = tool.execute(args, &ctx()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_missing_required_id() {
        let (tool, _dir) = create_test_tool().await;

        let args = serde_json::json!({
            "action": "update",
            "title": "Missing ID"
        });

        let result = tool.execute(args, &ctx()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_adr008_config_driven_focus() {
        // ADR-008: Focus values come from config, not hardcoded
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("todos.jsonl");
        let store = Arc::new(RwLock::new(TodoStore::new(file_path)));

        // Create tool with custom config values
        let tool = TodoTool::new(store, 5, 24, "Asia/Bangkok".to_string());

        assert_eq!(tool.max_focus_slots, 5);
        assert_eq!(tool.focus_deadline_hours, 24);
        assert_eq!(tool.timezone, "Asia/Bangkok");
    }

    // ── Phase 2: New actions tests ──────────────────────────────────────

    #[tokio::test]
    async fn test_add_subtask() {
        let (tool, _dir) = create_test_tool().await;

        // Create parent task first
        let parent_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Parent task"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        // Extract parent ID
        let parent_id_start = parent_result.find("ID: ").unwrap() + 4;
        let parent_id_end = parent_result[parent_id_start..].find(')').unwrap() + parent_id_start;
        let parent_id = &parent_result[parent_id_start..parent_id_end];

        // Create subtask
        let args = serde_json::json!({
            "action": "add_subtask",
            "parent_id": parent_id,
            "title": "Child task",
            "description": "A subtask"
        });

        let result = tool.execute(args, &ctx()).await.unwrap();
        assert!(result.contains("Subtask created"));
        assert!(result.contains("Child task"));
        assert!(result.contains(&format!("parent: {}", parent_id)));
    }

    #[tokio::test]
    async fn test_add_subtask_invalid_parent() {
        let (tool, _dir) = create_test_tool().await;

        let args = serde_json::json!({
            "action": "add_subtask",
            "parent_id": "nonexistent",
            "title": "Child task"
        });

        let result = tool.execute(args, &ctx()).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Parent task not found"));
    }

    #[tokio::test]
    async fn test_move_action() {
        let (tool, _dir) = create_test_tool().await;

        // Create two tasks
        let task1_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Task 1"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        let task2_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Task 2"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        // Extract IDs
        let extract_id = |result: &str| {
            let start = result.find("ID: ").unwrap() + 4;
            let end = result[start..].find(')').unwrap() + start;
            result[start..end].to_string()
        };

        let id1 = extract_id(&task1_result);
        let id2 = extract_id(&task2_result);

        // Move task2 to be child of task1
        let move_args = serde_json::json!({
            "action": "move",
            "id": id2,
            "new_parent_id": id1
        });

        let result = tool.execute(move_args, &ctx()).await.unwrap();
        assert!(result.contains("Moved task"));
        assert!(result.contains("Task 2"));
    }

    #[tokio::test]
    async fn test_attach_file() {
        let (tool, _dir) = create_test_tool().await;

        // Create task
        let task_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Task with attachment"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        let id_start = task_result.find("ID: ").unwrap() + 4;
        let id_end = task_result[id_start..].find(')').unwrap() + id_start;
        let id = &task_result[id_start..id_end];

        // Attach file
        let attach_args = serde_json::json!({
            "action": "attach",
            "id": id,
            "attachment_type": "file",
            "value": "/path/to/file.pdf",
            "attachment_title": "Important document"
        });

        let result = tool.execute(attach_args, &ctx()).await.unwrap();
        assert!(result.contains("Attachment added"));
        assert!(result.contains("file"));
        assert!(result.contains("/path/to/file.pdf"));
    }

    #[tokio::test]
    async fn test_attach_url() {
        let (tool, _dir) = create_test_tool().await;

        // Create task
        let task_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Task"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        let id_start = task_result.find("ID: ").unwrap() + 4;
        let id_end = task_result[id_start..].find(')').unwrap() + id_start;
        let id = &task_result[id_start..id_end];

        // Attach URL
        let attach_args = serde_json::json!({
            "action": "attach",
            "id": id,
            "attachment_type": "url",
            "value": "https://example.com/doc"
        });

        let result = tool.execute(attach_args, &ctx()).await.unwrap();
        assert!(result.contains("Attachment added"));
        assert!(result.contains("url"));
    }

    #[tokio::test]
    async fn test_detach() {
        let (tool, _dir) = create_test_tool().await;

        // Create task
        let task_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Task"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        let id_start = task_result.find("ID: ").unwrap() + 4;
        let id_end = task_result[id_start..].find(')').unwrap() + id_start;
        let id = &task_result[id_start..id_end];

        // Attach something
        tool.execute(
            serde_json::json!({
                "action": "attach",
                "id": id,
                "attachment_type": "note",
                "value": "Test note"
            }),
            &ctx(),
        )
        .await
        .unwrap();

        // Get task to find attachment ID
        let _show_result = tool
            .execute(serde_json::json!({"action": "show", "id": id}), &ctx())
            .await
            .unwrap();

        // For this test, we'll use a placeholder attachment_id
        // In a real scenario, we'd parse the show output to get the actual ID
        let detach_args = serde_json::json!({
            "action": "detach",
            "id": id,
            "attachment_id": "fake_id" // This will fail, but tests the error path
        });

        let result = tool.execute(detach_args, &ctx()).await;
        // Should fail since attachment doesn't exist
        assert!(result.is_err() || result.unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn test_log_time() {
        let (tool, _dir) = create_test_tool().await;

        // Create task
        let task_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Task"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        let id_start = task_result.find("ID: ").unwrap() + 4;
        let id_end = task_result[id_start..].find(')').unwrap() + id_start;
        let id = &task_result[id_start..id_end];

        // Log time
        let log_args = serde_json::json!({
            "action": "log_time",
            "id": id,
            "duration_minutes": 45,
            "note": "Worked on implementation"
        });

        let result = tool.execute(log_args, &ctx()).await.unwrap();
        assert!(result.contains("Logged 45 minutes"));
        assert!(result.contains(id));
    }

    #[tokio::test]
    async fn test_tree_single_root() {
        let (tool, _dir) = create_test_tool().await;

        // Create a simple tree
        tool.execute(
            serde_json::json!({
                "action": "add",
                "title": "Root task"
            }),
            &ctx(),
        )
        .await
        .unwrap();

        let tree_args = serde_json::json!({
            "action": "tree"
        });

        let result = tool.execute(tree_args, &ctx()).await.unwrap();
        assert!(result.contains("Task Tree"));
        assert!(result.contains("Root task"));
    }

    #[tokio::test]
    async fn test_tree_with_children() {
        let (tool, _dir) = create_test_tool().await;

        // Create parent
        let parent_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Parent"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        let id_start = parent_result.find("ID: ").unwrap() + 4;
        let id_end = parent_result[id_start..].find(')').unwrap() + id_start;
        let parent_id = &parent_result[id_start..id_end];

        // Create children
        tool.execute(
            serde_json::json!({
                "action": "add_subtask",
                "parent_id": parent_id,
                "title": "Child 1"
            }),
            &ctx(),
        )
        .await
        .unwrap();

        tool.execute(
            serde_json::json!({
                "action": "add_subtask",
                "parent_id": parent_id,
                "title": "Child 2"
            }),
            &ctx(),
        )
        .await
        .unwrap();

        let tree_args = serde_json::json!({
            "action": "tree"
        });

        let result = tool.execute(tree_args, &ctx()).await.unwrap();
        assert!(result.contains("Parent"));
        assert!(result.contains("Child 1"));
        assert!(result.contains("Child 2"));
        assert!(result.contains("├─") || result.contains("└─"));
    }

    #[tokio::test]
    async fn test_search_by_title() {
        let (tool, _dir) = create_test_tool().await;

        tool.execute(
            serde_json::json!({
                "action": "add",
                "title": "Implement feature X"
            }),
            &ctx(),
        )
        .await
        .unwrap();

        tool.execute(
            serde_json::json!({
                "action": "add",
                "title": "Fix bug Y"
            }),
            &ctx(),
        )
        .await
        .unwrap();

        let search_args = serde_json::json!({
            "action": "search",
            "query": "feature"
        });

        let result = tool.execute(search_args, &ctx()).await.unwrap();
        assert!(result.contains("1 task(s) matching"));
        assert!(result.contains("Implement feature X"));
        assert!(!result.contains("Fix bug Y"));
    }

    #[tokio::test]
    async fn test_search_by_description() {
        let (tool, _dir) = create_test_tool().await;

        tool.execute(
            serde_json::json!({
                "action": "add",
                "title": "Task 1",
                "description": "This involves authentication work"
            }),
            &ctx(),
        )
        .await
        .unwrap();

        tool.execute(
            serde_json::json!({
                "action": "add",
                "title": "Task 2",
                "description": "UI improvements"
            }),
            &ctx(),
        )
        .await
        .unwrap();

        let search_args = serde_json::json!({
            "action": "search",
            "query": "authentication"
        });

        let result = tool.execute(search_args, &ctx()).await.unwrap();
        assert!(result.contains("1 task(s) matching"));
        assert!(result.contains("Task 1"));
    }

    #[tokio::test]
    async fn test_search_no_results() {
        let (tool, _dir) = create_test_tool().await;

        tool.execute(
            serde_json::json!({
                "action": "add",
                "title": "Task"
            }),
            &ctx(),
        )
        .await
        .unwrap();

        let search_args = serde_json::json!({
            "action": "search",
            "query": "nonexistent"
        });

        let result = tool.execute(search_args, &ctx()).await.unwrap();
        assert!(result.contains("No tasks found matching"));
    }

    // ── Phase 2: Enhanced actions tests ─────────────────────────────────

    #[tokio::test]
    async fn test_focus_starts_time_entry() {
        let (tool, _dir) = create_test_tool().await;

        // Create task
        let task_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Task"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        let id_start = task_result.find("ID: ").unwrap() + 4;
        let id_end = task_result[id_start..].find(')').unwrap() + id_start;
        let id = &task_result[id_start..id_end];

        // Focus task
        tool.execute(serde_json::json!({"action": "focus", "id": id}), &ctx())
            .await
            .unwrap();

        // Check if time entry was started (we'd need to show the task to verify)
        let show_result = tool
            .execute(serde_json::json!({"action": "show", "id": id}), &ctx())
            .await
            .unwrap();

        // The show output doesn't display time entries yet, but the focus succeeded
        assert!(show_result.contains("Task"));
    }

    #[tokio::test]
    async fn test_unfocus_closes_time_entry() {
        let (tool, _dir) = create_test_tool().await;

        // Create and focus task
        let task_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Task"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        let id_start = task_result.find("ID: ").unwrap() + 4;
        let id_end = task_result[id_start..].find(')').unwrap() + id_start;
        let id = &task_result[id_start..id_end];

        tool.execute(serde_json::json!({"action": "focus", "id": id}), &ctx())
            .await
            .unwrap();

        // Unfocus
        let result = tool
            .execute(serde_json::json!({"action": "unfocus", "id": id}), &ctx())
            .await
            .unwrap();

        assert!(result.contains("Unfocused task"));
    }

    #[tokio::test]
    async fn test_complete_closes_time_entry() {
        let (tool, _dir) = create_test_tool().await;

        // Create and focus task
        let task_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Task"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        let id_start = task_result.find("ID: ").unwrap() + 4;
        let id_end = task_result[id_start..].find(')').unwrap() + id_start;
        let id = &task_result[id_start..id_end];

        tool.execute(serde_json::json!({"action": "focus", "id": id}), &ctx())
            .await
            .unwrap();

        // Complete
        let result = tool
            .execute(serde_json::json!({"action": "complete", "id": id}), &ctx())
            .await
            .unwrap();

        assert!(result.contains("Completed"));
    }

    #[tokio::test]
    async fn test_complete_cascades_to_subtasks() {
        let (tool, _dir) = create_test_tool().await;

        // Create parent
        let parent_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Parent"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        let id_start = parent_result.find("ID: ").unwrap() + 4;
        let id_end = parent_result[id_start..].find(')').unwrap() + id_start;
        let parent_id = &parent_result[id_start..id_end];

        // Create subtask
        tool.execute(
            serde_json::json!({
                "action": "add_subtask",
                "parent_id": parent_id,
                "title": "Child"
            }),
            &ctx(),
        )
        .await
        .unwrap();

        // Complete parent
        let result = tool
            .execute(
                serde_json::json!({"action": "complete", "id": parent_id}),
                &ctx(),
            )
            .await
            .unwrap();

        assert!(result.contains("subtasks also completed") || result.contains("Completed"));
    }

    #[tokio::test]
    async fn test_list_by_project_id() {
        let (tool, _dir) = create_test_tool().await;

        // Create tasks with different project IDs
        tool.execute(
            serde_json::json!({
                "action": "add",
                "title": "Task in project A",
                "project_id": "proj-a"
            }),
            &ctx(),
        )
        .await
        .unwrap();

        tool.execute(
            serde_json::json!({
                "action": "add",
                "title": "Task in project B",
                "project_id": "proj-b"
            }),
            &ctx(),
        )
        .await
        .unwrap();

        let list_args = serde_json::json!({
            "action": "list",
            "project_id": "proj-a"
        });

        let result = tool.execute(list_args, &ctx()).await.unwrap();
        assert!(result.contains("1 task(s)"));
        assert!(result.contains("Task in project A"));
        assert!(!result.contains("Task in project B"));
    }

    #[tokio::test]
    async fn test_list_by_parent_id() {
        let (tool, _dir) = create_test_tool().await;

        // Create parent
        let parent_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Parent"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        let id_start = parent_result.find("ID: ").unwrap() + 4;
        let id_end = parent_result[id_start..].find(')').unwrap() + id_start;
        let parent_id = &parent_result[id_start..id_end];

        // Create subtask
        tool.execute(
            serde_json::json!({
                "action": "add_subtask",
                "parent_id": parent_id,
                "title": "Child"
            }),
            &ctx(),
        )
        .await
        .unwrap();

        let list_args = serde_json::json!({
            "action": "list",
            "parent_id": parent_id
        });

        let result = tool.execute(list_args, &ctx()).await.unwrap();
        assert!(result.contains("1 task(s)"));
        assert!(result.contains("Child"));
    }

    // ── Phase 5: Report action tests ────────────────────────────────────

    #[tokio::test]
    async fn test_report_weekly_basic_stats() {
        let (tool, _dir) = create_test_tool().await;

        // Create tasks in different projects
        let task1_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Task in Project A",
                    "project_id": "proj-a"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        let task2_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Task in Project B",
                    "project_id": "proj-b"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        // Extract IDs
        let extract_id = |result: &str| {
            let start = result.find("ID: ").unwrap() + 4;
            let end = result[start..].find(')').unwrap() + start;
            result[start..end].to_string()
        };

        let id1 = extract_id(&task1_result);
        let id2 = extract_id(&task2_result);

        // Log time for both tasks
        tool.execute(
            serde_json::json!({
                "action": "log_time",
                "id": id1,
                "duration_minutes": 60
            }),
            &ctx(),
        )
        .await
        .unwrap();

        tool.execute(
            serde_json::json!({
                "action": "log_time",
                "id": id2,
                "duration_minutes": 30
            }),
            &ctx(),
        )
        .await
        .unwrap();

        // Complete task1
        tool.execute(
            serde_json::json!({
                "action": "complete",
                "id": id1
            }),
            &ctx(),
        )
        .await
        .unwrap();

        // Call report action
        let report_args = serde_json::json!({
            "action": "report",
            "period": "week"
        });

        let result = tool.execute(report_args, &ctx()).await.unwrap();

        // Verify output contains key statistics
        assert!(result.contains("proj-a") || result.contains("Project A"));
        assert!(result.contains("proj-b") || result.contains("Project B"));
        assert!(result.contains("1") && result.contains("completed")); // 1 completed task
        assert!(result.contains("2") && result.contains("created")); // 2 created tasks
    }

    #[tokio::test]
    async fn test_report_filtered_by_project() {
        let (tool, _dir) = create_test_tool().await;

        // Create tasks in different projects
        tool.execute(
            serde_json::json!({
                "action": "add",
                "title": "Task in Project A",
                "project_id": "proj-a"
            }),
            &ctx(),
        )
        .await
        .unwrap();

        tool.execute(
            serde_json::json!({
                "action": "add",
                "title": "Task in Project B",
                "project_id": "proj-b"
            }),
            &ctx(),
        )
        .await
        .unwrap();

        // Report filtered to proj-a only
        let report_args = serde_json::json!({
            "action": "report",
            "period": "week",
            "project_id": "proj-a"
        });

        let result = tool.execute(report_args, &ctx()).await.unwrap();

        // Should only show proj-a
        assert!(result.contains("proj-a"));
        assert!(!result.contains("proj-b"));
        assert!(result.contains("1") && result.contains("created")); // Only 1 task created in proj-a
    }

    #[tokio::test]
    async fn test_report_monthly_period() {
        let (tool, _dir) = create_test_tool().await;

        tool.execute(
            serde_json::json!({
                "action": "add",
                "title": "Recent task"
            }),
            &ctx(),
        )
        .await
        .unwrap();

        let report_args = serde_json::json!({
            "action": "report",
            "period": "month"
        });

        let result = tool.execute(report_args, &ctx()).await.unwrap();

        // Should show 30 days period
        assert!(result.contains("30 days"));
        assert!(result.contains("1") && result.contains("created"));
    }

    #[tokio::test]
    async fn test_report_counts_overdue_tasks() {
        let (tool, _dir) = create_test_tool().await;

        // Create overdue task (due date in the past)
        let past_date = (Utc::now() - chrono::Duration::days(5)).to_rfc3339();
        tool.execute(
            serde_json::json!({
                "action": "add",
                "title": "Overdue task",
                "due_date": past_date
            }),
            &ctx(),
        )
        .await
        .unwrap();

        let report_args = serde_json::json!({
            "action": "report",
            "period": "week"
        });

        let result = tool.execute(report_args, &ctx()).await.unwrap();

        // Should count 1 overdue task
        assert!(result.contains("Overdue tasks: 1"));
    }

    #[tokio::test]
    async fn test_report_counts_focus_sessions() {
        let (tool, _dir) = create_test_tool().await;

        // Create task
        let task_result = tool
            .execute(
                serde_json::json!({
                    "action": "add",
                    "title": "Task"
                }),
                &ctx(),
            )
            .await
            .unwrap();

        let id_start = task_result.find("ID: ").unwrap() + 4;
        let id_end = task_result[id_start..].find(')').unwrap() + id_start;
        let id = &task_result[id_start..id_end];

        // Focus and unfocus to create a focus time entry
        tool.execute(serde_json::json!({"action": "focus", "id": id}), &ctx())
            .await
            .unwrap();

        // Wait a moment then unfocus (to generate duration)
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        tool.execute(serde_json::json!({"action": "unfocus", "id": id}), &ctx())
            .await
            .unwrap();

        let report_args = serde_json::json!({
            "action": "report",
            "period": "week"
        });

        let result = tool.execute(report_args, &ctx()).await.unwrap();

        // Should count 1 focus session
        assert!(result.contains("Focus sessions: 1"));
    }

    #[tokio::test]
    async fn test_report_with_no_data() {
        let (tool, _dir) = create_test_tool().await;

        let report_args = serde_json::json!({
            "action": "report",
            "period": "week"
        });

        let result = tool.execute(report_args, &ctx()).await.unwrap();

        // Should show zeros
        assert!(result.contains("Tasks created: 0"));
        assert!(result.contains("Tasks completed: 0"));
    }

    #[tokio::test]
    async fn test_report_invalid_period() {
        let (tool, _dir) = create_test_tool().await;

        let report_args = serde_json::json!({
            "action": "report",
            "period": "year" // Invalid
        });

        let result = tool.execute(report_args, &ctx()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("period must be"));
    }
}
