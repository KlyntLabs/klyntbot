//! TodoTool - Tool interface for todo system
//!
//! Provides 24 actions for complete todo management through the Tool trait.
//! Sprint 5 adds: search_semantic, search_hybrid (semantic search with embeddings).

use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use super::{RoutingContext, Tool};
use crate::calendar_tool::CalendarHandler;
use crate::embedding_engine::EmbeddingHandler;
use crate::enrichment::EnrichmentHandler;
use crate::params::ParamExtractor;
use crate::rrule_utils;
use crate::todo_types::{
    Attachment, AttachmentType, TimeEntry, Todo, TodoFilter, TodoPatch, TodoStatus,
};
use common::utils::date::parse_datetime;
use common::{Result, ToolError};
use tracing::{debug, info, warn};

/// TodoTool with config-driven focus values (ADR-008)
pub struct TodoTool {
    repo: storage::TodoRepo,
    max_focus_slots: usize,
    focus_deadline_hours: u64,
    calendar_handler: Option<Arc<dyn CalendarHandler>>,
    enrichment_handler: Option<Arc<dyn EnrichmentHandler>>,
    embedding_handler: Option<Arc<dyn EmbeddingHandler>>,
    embedding_repo: Option<storage::EmbeddingRepo>,
    /// Default cosine similarity threshold for semantic search (from config)
    semantic_threshold: f64,
    /// RRF k parameter for hybrid search (from config)
    rrf_k: u32,
    timezone: String,
    /// Learning feedback handler for reporting enrichment outcomes
    feedback_handler: Option<Arc<dyn crate::EnrichmentFeedbackHandler>>,
}

impl TodoTool {
    /// Create a new TodoTool with config values
    pub fn new(
        repo: storage::TodoRepo,
        max_focus_slots: usize,
        focus_deadline_hours: u64,
        timezone: String,
    ) -> Self {
        Self {
            repo,
            max_focus_slots,
            focus_deadline_hours,
            calendar_handler: None,
            enrichment_handler: None,
            embedding_handler: None,
            embedding_repo: None,
            semantic_threshold: 0.5,
            rrf_k: 60,
            timezone,
            feedback_handler: None,
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

    /// Add learning feedback handler for reporting enrichment outcomes
    pub fn with_feedback_handler(
        mut self,
        handler: Arc<dyn crate::EnrichmentFeedbackHandler>,
    ) -> Self {
        self.feedback_handler = Some(handler);
        self
    }

    /// Add embedding handler for auto-generating embeddings on add/update
    pub fn with_embedding_handler(mut self, handler: Arc<dyn EmbeddingHandler>) -> Self {
        self.embedding_handler = Some(handler);
        self
    }

    /// Add embedding repo for SQL-backed semantic search
    pub fn with_embedding_repo(mut self, repo: storage::EmbeddingRepo) -> Self {
        self.embedding_repo = Some(repo);
        self
    }

    /// Set semantic search config values (threshold and RRF k)
    pub fn with_search_config(mut self, threshold: f64, rrf_k: u32) -> Self {
        self.semantic_threshold = threshold;
        self.rrf_k = rrf_k;
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

    /// Record enrichment feedback for all suggestion fields (priority, duration, due date).
    /// When `accepted_override` is `Some(true)`, all fields are marked accepted (manual enrich).
    /// When `None`, each field's confidence is checked against 0.7 threshold (auto-enrich).
    async fn record_enrichment_feedback(
        &self,
        task_id: &str,
        result: &crate::enrichment::EnrichmentResult,
        accepted_override: Option<bool>,
    ) {
        let Some(fb) = &self.feedback_handler else {
            return;
        };
        let now = chrono::Utc::now();

        // Collect (field, stringified_value, confidence) tuples eagerly to stay Send
        let fields: Vec<(&str, String, f64)> = [
            result
                .priority
                .as_ref()
                .map(|s| ("priority", s.value.to_string(), s.confidence)),
            result
                .estimated_minutes
                .as_ref()
                .map(|s| ("estimated_minutes", s.value.to_string(), s.confidence)),
            result
                .due_date
                .as_ref()
                .map(|s| ("due_date", s.value.to_string(), s.confidence)),
        ]
        .into_iter()
        .flatten()
        .collect();

        for (field, suggested_value, conf) in fields {
            let accepted = accepted_override.unwrap_or(conf >= 0.7);
            let _ = fb
                .record_feedback(crate::EnrichmentFeedbackEntry {
                    task_id: task_id.to_string(),
                    field: field.to_string(),
                    suggested_value,
                    actual_value: None,
                    accepted,
                    confidence: conf,
                    timestamp: now,
                })
                .await;
        }
    }

    // ─── Daily Planning — Scoring Algorithm (Sprint 6, Task #9/#11) ───

    /// Calculate urgency tier based on due date relative to now.
    fn calculate_urgency(
        due_date: Option<chrono::DateTime<chrono::Utc>>,
        now: chrono::DateTime<chrono::Utc>,
    ) -> u32 {
        match due_date {
            None => 1,
            Some(due) => {
                let now_date = now.date_naive();
                let due_date = due.date_naive();
                if due_date < now_date {
                    10 // Overdue
                } else if due_date == now_date {
                    5 // Due today
                } else if due_date == now_date + chrono::Duration::days(1) {
                    3 // Due tomorrow
                } else {
                    1 // Future
                }
            }
        }
    }

    /// Convert priority to weight for scoring.
    fn priority_weight(priority: Option<u8>) -> u32 {
        match priority {
            Some(1) => 5,
            Some(2) => 4,
            Some(3) => 3,
            Some(4) => 2,
            Some(5) => 1,
            None => 3,
            _ => 3,
        }
    }

    /// Calculate age in days from created_at to now.
    fn calculate_age_days(
        created_at: chrono::DateTime<chrono::Utc>,
        now: chrono::DateTime<chrono::Utc>,
    ) -> f64 {
        let duration = now.signed_duration_since(created_at);
        (duration.num_seconds().max(0) as f64) / 86400.0
    }

    /// Calculate composite planning score for a task.
    fn calculate_score(task: &Todo, now: chrono::DateTime<chrono::Utc>) -> f64 {
        let urgency = Self::calculate_urgency(task.due_date, now) as f64;
        let priority_wt = Self::priority_weight(task.priority) as f64;
        let age_days = Self::calculate_age_days(task.created_at, now);
        (urgency * priority_wt) + (age_days * 0.1)
    }

    // ─── Helper: convert row to domain Todo with deps/attachments/time_entries ───

    /// Load a full domain Todo from a row, including related data.
    async fn load_full_todo(&self, row: storage::TodoRow) -> Result<Todo> {
        let id = row.id.clone();
        let mut todo = Todo::from(row);

        // Load attachments
        let att_rows = self.repo.list_attachments(&id).await?;
        todo.attachments = att_rows.into_iter().map(Attachment::from).collect();

        // Load time entries
        let te_rows = self.repo.list_time_entries(&id).await?;
        todo.time_entries = te_rows.into_iter().map(TimeEntry::from).collect();

        // Load dependency edges
        let blockers = self.repo.get_blockers(&id).await?;
        todo.blocked_by = blockers.into_iter().map(|r| r.id).collect();
        let blocking = self.repo.get_blocking(&id).await?;
        todo.blocks = blocking.into_iter().map(|r| r.id).collect();

        Ok(todo)
    }

    /// Load a domain Todo by ID, or return None.
    async fn get_full_todo(&self, id: &str) -> Result<Option<Todo>> {
        match self.repo.get(id).await? {
            Some(row) => Ok(Some(self.load_full_todo(row).await?)),
            None => Ok(None),
        }
    }
}

#[async_trait]
impl Tool for TodoTool {
    fn name(&self) -> &str {
        "todo"
    }

    fn description(&self) -> &str {
        "Manage tasks and todos. Actions: add, list, update, complete, delete, show, summary, focus, unfocus, add_subtask, move, attach, detach, log_time, tree, search, search_semantic, search_hybrid, report, add_dependency, remove_dependency, recur, list_recurring, delete_recurring, enrich, plan."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["add", "list", "update", "complete", "delete", "show", "summary", "focus", "unfocus", "add_subtask", "move", "attach", "detach", "log_time", "tree", "search", "search_semantic", "search_hybrid", "report", "add_dependency", "remove_dependency", "recur", "list_recurring", "delete_recurring", "enrich", "plan"],
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
                    "description": "Search query (for search/search_semantic/search_hybrid)"
                },
                "threshold": {
                    "type": "number",
                    "description": "Similarity threshold for semantic search (0.0-1.0, default: config value)"
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
                },
                "count": {
                    "type": "integer",
                    "description": "Number of tasks to include in plan (for plan, default: 3)"
                },
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let p = ParamExtractor::new(&args);
        let action = p.required_str("action")?;

        match action {
            "add" => {
                let title = p.required_str("title")?;

                let mut todo = Todo::default_instance();
                todo.title = title.to_string();
                todo.description = p.optional_str("description")?.map(String::from);
                todo.priority = p.optional_u64("priority")?.map(|v| v as u8);
                todo.due_date = p
                    .optional_str("due_date")?
                    .and_then(|s| parse_datetime(s, &self.timezone));
                todo.tags = p.string_array_or_empty("tags")?;
                todo.project_id = p.optional_str("project_id")?.map(String::from);

                let row = storage::TodoRow::from(&todo);
                let created_row = self.repo.add(&row).await?;
                let created = Todo::from(created_row);

                // Auto-enrich if handler is available
                let mut enriched_info = String::new();
                if let Some(handler) = &self.enrichment_handler {
                    match handler.enrich_task(&created).await {
                        Ok(Some(suggestions)) => {
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
                                let storage_patch = patch.to_storage_patch(&created.id);
                                if self.repo.update(&storage_patch).await.is_ok() {
                                    enriched_info = format!(" (enriched: {})", applied.join(", "));
                                }
                            }

                            self.record_enrichment_feedback(&created.id, &suggestions, None)
                                .await;
                        }
                        Ok(None) => {}
                        Err(e) => {
                            warn!("Task enrichment failed: {}", e);
                        }
                    }
                }

                // Auto-embed (best-effort)
                if let Some(ref emb) = self.embedding_handler {
                    if let Err(e) = emb.embed_todo(&created).await {
                        warn!(
                            "Failed to generate embedding for todo {}: {}",
                            created.id, e
                        );
                    }
                }

                self.trigger_sync_async().await;

                Ok(format!(
                    "Task created: {} (ID: {}){}",
                    created.title, created.id, enriched_info
                ))
            }

            "list" => {
                let filter = TodoFilter {
                    status: p
                        .optional_str("status")?
                        .and_then(TodoStatus::from_str_loose),
                    priority_min: p.optional_u64("priority_min")?.map(|v| v as u8),
                    tag: p.optional_str("tag")?.map(String::from),
                    limit: p.optional_u64("limit")?.map(|l| l as usize),
                    project_id: p.optional_str("project_id")?.map(String::from),
                    parent_id: p.optional_str("parent_id")?.map(String::from),
                    include_templates: false,
                };

                let rows = self.repo.list(&filter.to_storage_filter()).await?;
                let todos: Vec<Todo> = rows.into_iter().map(Todo::from).collect();

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
                    status: p
                        .optional_str("status")?
                        .and_then(TodoStatus::from_str_loose),
                    last_reminded_at: None,
                    calendar_event_uid: None,
                    estimated_minutes: p.optional_u64("estimated_minutes")?.map(|v| Some(v as u32)),
                };

                let storage_patch = patch.to_storage_patch(id);
                match self.repo.update(&storage_patch).await {
                    Ok(row) => {
                        let todo = Todo::from(row);

                        // Auto-embed updated task (best-effort)
                        if let Some(ref emb) = self.embedding_handler {
                            if let Err(e) = emb.embed_todo(&todo).await {
                                warn!("Failed to regenerate embedding for todo {}: {}", todo.id, e);
                            }
                        }

                        self.trigger_sync_async().await;
                        Ok(format!("Updated task: {}", todo.title))
                    }
                    Err(storage::StorageError::NotFound(msg)) => {
                        Err(ToolError::ExecutionFailed(format!("Task not found: {}", msg)).into())
                    }
                    Err(e) => Err(e.into()),
                }
            }

            "complete" => {
                let id = p.required_str("id")?;

                // Check for incomplete blockers before allowing completion
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

                // Close any running time entries before completing
                let te_rows = self.repo.list_time_entries(id).await?;
                for entry in &te_rows {
                    if entry.ended_at.is_none() {
                        self.repo.close_time_entry(id, entry.id).await?;
                    }
                }

                // Collect tasks this will unblock (before marking done)
                let blocking_rows = self.repo.get_blocking(id).await?;
                let blocks_ids: Vec<String> = blocking_rows.iter().map(|r| r.id.clone()).collect();

                // Mark as done
                let done_patch = TodoPatch {
                    status: Some(TodoStatus::Done),
                    ..Default::default()
                };
                let storage_patch = done_patch.to_storage_patch(id);

                let result = match self.repo.update(&storage_patch).await {
                    Ok(row) => {
                        let todo = Todo::from(row);

                        // Cascade completion to subtasks
                        let completed_count = self.repo.cascade_complete(id).await?;

                        let mut msg = if completed_count == 0 {
                            format!("Completed: {}", todo.title)
                        } else {
                            format!(
                                "Completed: {} ({} subtasks also completed)",
                                todo.title, completed_count
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

                        Ok(msg)
                    }
                    Err(storage::StorageError::NotFound(msg)) => {
                        Err(ToolError::ExecutionFailed(format!("Task not found: {}", msg)).into())
                    }
                    Err(e) => Err(e.into()),
                };

                if result.is_ok() {
                    self.trigger_sync_async().await;
                }

                result
            }

            "delete" => {
                let id = p.required_str("id")?;

                let result = if self.repo.delete(id).await? {
                    Ok(format!("Deleted task: {}", id))
                } else {
                    Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into())
                };

                if result.is_ok() {
                    self.trigger_sync_async().await;
                }

                result
            }

            "enrich" => {
                let id = p.required_str("id")?;

                let task = self
                    .get_full_todo(id)
                    .await?
                    .ok_or_else(|| ToolError::ExecutionFailed(format!("Task not found: {}", id)))?;

                let handler = match &self.enrichment_handler {
                    Some(h) => h,
                    None => {
                        return Err(ToolError::ExecutionFailed(
                            "Enrichment is not enabled".to_string(),
                        )
                        .into());
                    }
                };

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

                        let storage_patch = patch.to_storage_patch(id);
                        self.repo.update(&storage_patch).await?;

                        self.record_enrichment_feedback(id, &result, Some(true))
                            .await;

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

                match self.get_full_todo(id).await? {
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
                let summary = self.repo.summary().await?;
                let mut output = format!("Total tasks: {}\n\n", summary.total);
                output.push_str("By status:\n");
                output.push_str(&format!("  Todo: {}\n", summary.todo));
                output.push_str(&format!("  Doing: {}\n", summary.doing));
                output.push_str(&format!("  Done: {}\n", summary.done));
                let overdue = self.repo.overdue().await?;
                if !overdue.is_empty() {
                    output.push_str(&format!("\nOverdue: {} tasks\n", overdue.len()));
                }
                Ok(output)
            }

            "focus" => {
                let id = p.required_str("id")?;

                let deadline = Some(
                    Utc::now()
                        + chrono::Duration::hours(self.focus_deadline_hours as i64),
                );
                if self
                    .repo
                    .focus(id, self.max_focus_slots as i64, deadline)
                    .await?
                {
                    // Auto-start TimeEntry on focus
                    let te_rows = self.repo.list_time_entries(id).await?;
                    let has_running_entry = te_rows.iter().any(|e| e.ended_at.is_none());

                    if !has_running_entry {
                        self.repo
                            .add_time_entry(id, "focus", Utc::now(), None, None)
                            .await?;
                    }
                    Ok(format!("Focused on task: {}", id))
                } else {
                    Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into())
                }
            }

            "unfocus" => {
                let id = p.required_str("id")?;

                // Auto-close any running time entries before unfocusing
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

            "add_subtask" => {
                let title = p.required_str("title")?;
                let parent_id = p.required_str("parent_id")?;

                // Verify parent exists
                if self.repo.get(parent_id).await?.is_none() {
                    return Err(ToolError::InvalidParams(format!(
                        "Parent task not found: {}",
                        parent_id
                    ))
                    .into());
                }

                let mut todo = Todo::default_instance();
                todo.title = title.to_string();
                todo.description = p.optional_str("description")?.map(String::from);
                todo.priority = p.optional_u64("priority")?.map(|v| v as u8);
                todo.due_date = p
                    .optional_str("due_date")?
                    .and_then(|s| parse_datetime(s, &self.timezone));
                todo.tags = p.string_array_or_empty("tags")?;
                todo.parent_id = Some(parent_id.to_string());
                todo.project_id = p.optional_str("project_id")?.map(String::from);

                let row = storage::TodoRow::from(&todo);
                let created_row = self.repo.add(&row).await?;
                let created = Todo::from(created_row);
                let result = format!(
                    "Subtask created: {} (ID: {}, parent: {})",
                    created.title, created.id, parent_id
                );

                self.trigger_sync_async().await;

                Ok(result)
            }

            "move" => {
                let id = p.required_str("id")?;

                let new_parent_id = p.optional_str("new_parent_id")?.map(String::from);
                let new_project_id = p.optional_str("new_project_id")?.map(String::from);

                // Verify new parent exists if specified
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
                    .move_todo(id, new_parent_id.as_deref(), new_project_id.as_deref())
                    .await
                {
                    Ok(row) => {
                        let todo = Todo::from(row);
                        let mut parts = vec![format!("Moved task: {}", todo.title)];
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

                if result.is_ok() {
                    self.trigger_sync_async().await;
                }

                result
            }

            "attach" => {
                let id = p.required_str("id")?;
                let attachment_type_str = p.required_str("attachment_type")?;
                let value = p.required_str("value")?;

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

                let att_type_str = match attachment_type {
                    AttachmentType::File => "file",
                    AttachmentType::Url => "url",
                    AttachmentType::Note => "note",
                };
                let title_opt = p.optional_str("attachment_title")?;
                let tags: Vec<String> = Vec::new();

                self.repo
                    .add_attachment(id, att_type_str, value, title_opt, &tags)
                    .await?;
                Ok(format!(
                    "Attachment added to task: {} ({}: {})",
                    id, attachment_type_str, value
                ))
            }

            "detach" => {
                let id = p.required_str("id")?;
                let attachment_id_str = p.required_str("attachment_id")?;
                let attachment_id = uuid::Uuid::parse_str(attachment_id_str).map_err(|_| {
                    ToolError::InvalidParams(format!(
                        "Invalid attachment ID: {}",
                        attachment_id_str
                    ))
                })?;

                if self.repo.remove_attachment(id, attachment_id).await? {
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

            "tree" => {
                // Build hierarchical tree view of todos
                let rows = self.repo.list(&storage::TodoFilter::default()).await?;

                // Load full todos with deps for the tree
                let mut todos: Vec<Todo> = Vec::with_capacity(rows.len());
                for row in rows {
                    todos.push(self.load_full_todo(row).await?);
                }

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

                    let children: Vec<_> = all_todos
                        .iter()
                        .filter(|t| t.parent_id.as_ref() == Some(&todo.id))
                        .collect();

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
                let query = p.required_str("query")?;

                let rows = self.repo.search_by_keyword(query).await?;
                let results: Vec<Todo> = rows.into_iter().map(Todo::from).collect();

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

            "search_semantic" => {
                let query = p.required_str("query")?;
                let query = query.trim();
                let limit = p.optional_u64("limit")?.unwrap_or(10) as usize;
                let threshold = p
                    .optional_f64("threshold")?
                    .unwrap_or(self.semantic_threshold);

                // Validation
                if query.is_empty() {
                    return Err(ToolError::InvalidParams(
                        "Search query cannot be empty".to_string(),
                    )
                    .into());
                }
                if query.len() > 1000 {
                    return Err(ToolError::InvalidParams(
                        "Query too long (max 1000 characters)".to_string(),
                    )
                    .into());
                }
                if !(0.0..=1.0).contains(&threshold) {
                    return Err(ToolError::InvalidParams(
                        "Threshold must be between 0.0 and 1.0".to_string(),
                    )
                    .into());
                }
                if limit == 0 || limit > 1000 {
                    return Err(ToolError::InvalidParams(
                        "Limit must be between 1 and 1000".to_string(),
                    )
                    .into());
                }

                let search_start = std::time::Instant::now();

                let emb = self.embedding_handler.as_ref().ok_or_else(|| {
                    ToolError::ExecutionFailed(
                        "Semantic search unavailable: embedding engine not initialized. Use plain 'search' for keyword search.".to_string(),
                    )
                })?;

                let emb_repo = self.embedding_repo.as_ref().ok_or_else(|| {
                    ToolError::ExecutionFailed(
                        "Semantic search unavailable: embedding repo not configured".to_string(),
                    )
                })?;

                // Get total task count for partial-coverage note
                let total_summary = self.repo.summary().await?;
                let total_tasks = total_summary.total as usize;

                // 1. Embed query
                debug!(
                    query = query,
                    "Generating query embedding for semantic search"
                );
                let query_vec = emb.embed_query(query).await?;

                // 2. Search via pgvector (cosine similarity in SQL — much faster than brute-force)
                let scored = emb_repo
                    .search_similar_vec(&query_vec, limit as i64, threshold)
                    .await?;

                let embedding_count = emb_repo.count().await.unwrap_or(0) as usize;

                if scored.is_empty() {
                    let mut msg = format!(
                        "No semantically similar tasks found for '{}' (threshold: {:.2}).",
                        query, threshold,
                    );
                    if threshold > 0.7 {
                        msg.push_str(" Try lowering the threshold.");
                    }
                    return Ok(msg);
                }

                // 5. Join with repo to get full Todo objects
                let mut output = format!(
                    "{} task(s) matching '{}' (semantic, threshold: {:.2}):\n",
                    scored.len(),
                    query,
                    threshold,
                );

                for (id, sim) in &scored {
                    if let Ok(Some(row)) = self.repo.get(id).await {
                        let todo = Todo::from(row);
                        output.push_str(&format!(
                            "\n- [{}] {} (P{}, {:?}, sim: {:.2})",
                            todo.id,
                            todo.title,
                            todo.priority.unwrap_or(3),
                            todo.status,
                            sim,
                        ));
                    }
                }

                if embedding_count < total_tasks {
                    output.push_str(&format!(
                        "\n\nNote: {} of {} tasks have embeddings. Run `todo backfill-embeddings` to index all tasks.",
                        embedding_count, total_tasks,
                    ));
                }

                info!(
                    query = query,
                    results = scored.len(),
                    embeddings_searched = embedding_count,
                    elapsed_ms = search_start.elapsed().as_millis() as u64,
                    "Semantic search completed"
                );

                Ok(output)
            }

            "search_hybrid" => {
                let query = p.required_str("query")?;
                let query_trimmed = query.trim();
                let limit = p.optional_u64("limit")?.unwrap_or(10) as usize;

                if query_trimmed.is_empty() {
                    return Err(ToolError::InvalidParams(
                        "Search query cannot be empty".to_string(),
                    )
                    .into());
                }
                if query_trimmed.len() > 1000 {
                    return Err(ToolError::InvalidParams(
                        "Query too long (max 1000 characters)".to_string(),
                    )
                    .into());
                }
                if limit == 0 || limit > 1000 {
                    return Err(ToolError::InvalidParams(
                        "Limit must be between 1 and 1000".to_string(),
                    )
                    .into());
                }

                let search_start = std::time::Instant::now();

                // 1. Run keyword search
                let keyword_rows = self.repo.search_by_keyword(query_trimmed).await?;
                let keyword_results: Vec<Todo> =
                    keyword_rows.into_iter().map(Todo::from).collect();

                // Build ID-to-Todo map for RRF lookups
                let all_rows = self.repo.list(&storage::TodoFilter::default()).await?;
                let all_todos: Vec<Todo> = all_rows.into_iter().map(Todo::from).collect();
                let todos_by_id: HashMap<String, crate::search_utils::SearchResult> = all_todos
                    .into_iter()
                    .map(|t| {
                        (
                            t.id.clone(),
                            crate::search_utils::SearchResult::Todo(Box::new(t)),
                        )
                    })
                    .collect();

                // 2. Run semantic search via pgvector (if available)
                let semantic_results: Vec<(String, f64)> = if let (Some(emb), Some(emb_repo)) =
                    (&self.embedding_handler, &self.embedding_repo)
                {
                    let query_vec = emb.embed_query(query_trimmed).await?;
                    emb_repo
                        .search_similar_vec(&query_vec, 100, self.semantic_threshold)
                        .await
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };

                // 3. Merge via Reciprocal Rank Fusion
                let keyword_count = keyword_results.len();
                let keyword_search_results: Vec<crate::search_utils::SearchResult> =
                    keyword_results
                        .into_iter()
                        .map(|t| crate::search_utils::SearchResult::Todo(Box::new(t)))
                        .collect();

                let merged = crate::search_utils::rrf_merge(
                    &keyword_search_results,
                    &semantic_results,
                    self.rrf_k,
                    &todos_by_id,
                );

                if merged.is_empty() {
                    return Ok(format!(
                        "No tasks found matching '{}' (hybrid: keyword + semantic).",
                        query_trimmed,
                    ));
                }

                let results: Vec<_> = merged.into_iter().take(limit).collect();
                let mut output = format!(
                    "{} task(s) matching '{}' (hybrid: keyword + semantic):\n",
                    results.len(),
                    query_trimmed,
                );

                for (search_result, _rrf_score, source) in &results {
                    if let crate::search_utils::SearchResult::Todo(todo) = search_result {
                        output.push_str(&format!(
                            "\n- [{}] {} (P{}, {:?}, match: {})",
                            todo.id,
                            todo.title,
                            todo.priority.unwrap_or(3),
                            todo.status,
                            source,
                        ));
                    }
                }

                let has_semantic = self.embedding_handler.is_some();
                info!(
                    query = query_trimmed,
                    results = results.len(),
                    keyword_results = keyword_count,
                    semantic_available = has_semantic,
                    elapsed_ms = search_start.elapsed().as_millis() as u64,
                    "Hybrid search completed"
                );

                Ok(output)
            }

            "report" => {
                let period = p.required_str("period")?;
                let project_id_filter = p.optional_str("project_id")?.map(String::from);

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

                // Get all todos (with related data for time entries)
                let rows = self.repo.list(&storage::TodoFilter::default()).await?;
                let mut todos: Vec<Todo> = Vec::with_capacity(rows.len());
                for row in rows {
                    todos.push(self.load_full_todo(row).await?);
                }

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
                let mut projects: HashMap<String, Vec<&Todo>> = HashMap::new();
                for todo in &todos {
                    let project_key = todo
                        .project_id
                        .clone()
                        .unwrap_or_else(|| "(no project)".to_string());
                    projects.entry(project_key).or_default().push(todo);
                }

                let mut completed_in_period_total = 0;
                let mut created_in_period_total = 0;
                let mut total_time_secs = 0u64;
                let mut focus_session_count = 0;
                let mut focus_time_secs = 0u64;
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

                for todo in &todos {
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

                    if let Some(due) = todo.due_date {
                        if due < now && todo.status != TodoStatus::Done {
                            overdue_count += 1;
                        }
                    }
                }

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

                self.repo.add_dependency(task_id, blocked_by).await?;

                let task = self.repo.get(task_id).await?.unwrap();
                let blocker = self.repo.get(blocked_by).await?.unwrap();
                Ok(format!(
                    "Dependency added: [{}] {} is now blocked by [{}] {}",
                    task.id, task.title, blocker.id, blocker.title
                ))
            }

            "remove_dependency" => {
                let task_id = p.required_str("task_id")?;
                let blocked_by = p.required_str("blocked_by")?;

                self.repo.remove_dependency(task_id, blocked_by).await?;

                Ok(format!(
                    "Dependency removed: {} is no longer blocked by {}",
                    task_id, blocked_by
                ))
            }

            "recur" => {
                let title = p.required_str("title")?;
                let rule = p.required_str("rule")?;

                rrule_utils::validate_rrule(rule)?;

                let now = Utc::now();
                let next_date = rrule_utils::next_occurrence(rule, now)?;

                let mut template = Todo::default_instance();
                template.title = title.to_string();
                template.description = p.optional_str("description")?.map(String::from);
                template.priority = p.optional_u64("priority")?.map(|v| v as u8);
                template.tags = p.string_array_or_empty("tags")?;
                template.project_id = p.optional_str("project_id")?.map(String::from);
                template.recurrence_rule = Some(rule.to_string());
                template.is_template = true;
                template.next_instance_date = next_date;

                let row = storage::TodoRow::from(&template);
                let created_row = self.repo.add(&row).await?;
                let created = Todo::from(created_row);
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
                let rows = self.repo.list_templates().await?;
                let templates: Vec<Todo> = rows.into_iter().map(Todo::from).collect();

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

                let template = self.repo.get(template_id).await?;
                match template {
                    Some(t) if t.is_template => {
                        self.repo.delete(template_id).await?;
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

            "plan" => {
                let count = p.optional_u64("count")?.unwrap_or(3).min(10) as usize;
                info!("Generating daily plan (top {} tasks)", count);

                let rows = self.repo.list(&storage::TodoFilter::default()).await?;
                let all_tasks: Vec<Todo> = rows.into_iter().map(Todo::from).collect();
                debug!("Total tasks in store: {}", all_tasks.len());

                let now = Utc::now();
                let mut eligible = Vec::new();
                let mut filtered_counts = std::collections::HashMap::new();

                for task in all_tasks {
                    if task.status != TodoStatus::Todo {
                        *filtered_counts.entry("non_todo").or_insert(0) += 1;
                        continue;
                    }
                    if task.is_template {
                        *filtered_counts.entry("template").or_insert(0) += 1;
                        continue;
                    }
                    if task.focused_at.is_some() {
                        *filtered_counts.entry("focused").or_insert(0) += 1;
                        continue;
                    }

                    let blockers = self.repo.incomplete_blockers(&task.id).await?;
                    if !blockers.is_empty() {
                        *filtered_counts.entry("blocked").or_insert(0) += 1;
                        continue;
                    }

                    eligible.push(task);
                }

                debug!(
                    "Eligible tasks: {} (filtered: {} non-todo, {} templates, {} focused, {} blocked)",
                    eligible.len(),
                    filtered_counts.get("non_todo").unwrap_or(&0),
                    filtered_counts.get("template").unwrap_or(&0),
                    filtered_counts.get("focused").unwrap_or(&0),
                    filtered_counts.get("blocked").unwrap_or(&0)
                );

                let mut scored: Vec<(Todo, f64)> = eligible
                    .into_iter()
                    .map(|task| {
                        let score = Self::calculate_score(&task, now);
                        debug!("Scored task [{}] '{}': {:.2}", task.id, task.title, score);
                        (task, score)
                    })
                    .collect();

                scored.sort_by(|a, b| {
                    b.1.partial_cmp(&a.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.0.created_at.cmp(&b.0.created_at))
                });

                let plan: Vec<&(Todo, f64)> = scored.iter().take(count).collect();

                if plan.is_empty() {
                    info!("Daily plan generated: No eligible tasks (all clear)");
                    return Ok("No eligible tasks for planning. All clear!".to_string());
                }

                info!(
                    "Daily plan generated: {} tasks (top scores: {:.1}, {:.1}, {:.1})",
                    plan.len(),
                    plan.first().map(|(_, s)| s).unwrap_or(&0.0),
                    plan.get(1).map(|(_, s)| s).unwrap_or(&0.0),
                    plan.get(2).map(|(_, s)| s).unwrap_or(&0.0)
                );

                let mut output = format!("Daily plan ({} tasks):\n\n", plan.len());
                for (i, (task, score)) in plan.iter().enumerate() {
                    let priority_label = task
                        .priority
                        .map(|p| format!("P{}", p))
                        .unwrap_or_else(|| "P3".to_string());

                    let due_context = if let Some(due) = task.due_date {
                        let due_date = due.date_naive();
                        let now_date = now.date_naive();

                        if due_date < now_date {
                            let days_overdue = (now_date - due_date).num_days();
                            format!("Overdue by {} day(s)", days_overdue)
                        } else if due_date == now_date {
                            "Due today".to_string()
                        } else if due_date == now_date + chrono::Duration::days(1) {
                            "Due tomorrow".to_string()
                        } else {
                            format!("Due {}", due_date.format("%Y-%m-%d"))
                        }
                    } else {
                        "No deadline".to_string()
                    };

                    let duration_info = task
                        .estimated_minutes
                        .map(|m| format!(" · {} min", m))
                        .unwrap_or_default();

                    output.push_str(&format!(
                        "{}. {} (ID: {})\n   {} · {} · Score: {:.1}{}\n\n",
                        i + 1,
                        task.title,
                        task.id,
                        priority_label,
                        due_context,
                        score,
                        duration_info
                    ));
                }

                Ok(output)
            }

            _ => Err(ToolError::InvalidParams(format!("Unknown action: {}", action)).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Scoring algorithm tests (no database needed)

    #[test]
    fn test_urgency_overdue_returns_10() {
        use chrono::{Duration, Utc};
        let now = Utc::now();
        let overdue = now - Duration::days(2);
        let urgency = TodoTool::calculate_urgency(Some(overdue), now);
        assert_eq!(urgency, 10);
    }

    #[test]
    fn test_urgency_today_returns_5() {
        use chrono::Utc;
        let now = Utc::now();
        let urgency = TodoTool::calculate_urgency(Some(now), now);
        assert_eq!(urgency, 5);
    }

    #[test]
    fn test_urgency_tomorrow_returns_3() {
        use chrono::{Duration, Utc};
        let now = Utc::now();
        let tomorrow = now + Duration::days(1);
        let urgency = TodoTool::calculate_urgency(Some(tomorrow), now);
        assert_eq!(urgency, 3);
    }

    #[test]
    fn test_urgency_future_returns_1() {
        use chrono::{Duration, Utc};
        let now = Utc::now();
        let future = now + Duration::days(7);
        let urgency = TodoTool::calculate_urgency(Some(future), now);
        assert_eq!(urgency, 1);
    }

    #[test]
    fn test_urgency_no_due_date_returns_1() {
        use chrono::Utc;
        let now = Utc::now();
        let urgency = TodoTool::calculate_urgency(None, now);
        assert_eq!(urgency, 1);
    }

    #[test]
    fn test_priority_weight_p1_returns_5() {
        assert_eq!(TodoTool::priority_weight(Some(1)), 5);
    }

    #[test]
    fn test_priority_weight_p2_returns_4() {
        assert_eq!(TodoTool::priority_weight(Some(2)), 4);
    }

    #[test]
    fn test_priority_weight_p3_returns_3() {
        assert_eq!(TodoTool::priority_weight(Some(3)), 3);
    }

    #[test]
    fn test_priority_weight_none_returns_3() {
        assert_eq!(TodoTool::priority_weight(None), 3);
    }

    #[test]
    fn test_score_formula_correct() {
        use chrono::{Duration, Utc};
        let now = Utc::now();
        let mut task = Todo::default_instance();
        task.id = "test-1".to_string();
        task.title = "Overdue P1 task".to_string();
        task.priority = Some(1);
        task.due_date = Some(now - Duration::days(2));
        task.created_at = now - Duration::days(5);
        task.status = TodoStatus::Todo;

        let score = TodoTool::calculate_score(&task, now);
        assert!(
            (score - 50.5).abs() < 0.01,
            "Score should be ~50.5, got {}",
            score
        );
    }

    #[test]
    fn test_age_calculation() {
        use chrono::{Duration, Utc};
        let now = Utc::now();
        let created_10_days_ago = now - Duration::days(10);
        let age = TodoTool::calculate_age_days(created_10_days_ago, now);
        assert!((age - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_age_calculation_future_timestamp() {
        use chrono::{Duration, Utc};
        let now = Utc::now();
        let created_in_future = now + Duration::days(5);
        let age = TodoTool::calculate_age_days(created_in_future, now);
        assert_eq!(age, 0.0);
    }

    // Integration tests requiring a PgPool are in tests/storage_integration_test.rs
}
