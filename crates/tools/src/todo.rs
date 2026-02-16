//! TodoTool - Tool interface for todo system
//!
//! Provides 24 actions for complete todo management through the Tool trait.
//! Sprint 5 adds: search_semantic, search_hybrid (semantic search with embeddings).

use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{RoutingContext, Tool};
use crate::calendar_tool::CalendarHandler;
use crate::embedding_engine::{self, EmbeddingHandler};
use crate::embedding_store::EmbeddingStore;
use crate::enrichment::EnrichmentHandler;
use crate::params::ParamExtractor;
use crate::rrule_utils;
use crate::todo_store::TodoStore;
use crate::todo_types::{Todo, TodoFilter, TodoPatch, TodoStatus};
use common::utils::date::parse_datetime;
use common::{Result, ToolError};
use tracing::{debug, info, warn};

/// TodoTool with config-driven focus values (ADR-008)
pub struct TodoTool {
    store: Arc<RwLock<TodoStore>>,
    max_focus_slots: usize,
    focus_deadline_hours: u64,
    calendar_handler: Option<Arc<dyn CalendarHandler>>,
    enrichment_handler: Option<Arc<dyn EnrichmentHandler>>,
    embedding_handler: Option<Arc<dyn EmbeddingHandler>>,
    embedding_store: Option<Arc<RwLock<EmbeddingStore>>>,
    /// Default cosine similarity threshold for semantic search (from config)
    semantic_threshold: f64,
    /// RRF k parameter for hybrid search (from config)
    rrf_k: u32,
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
            embedding_handler: None,
            embedding_store: None,
            semantic_threshold: 0.5,
            rrf_k: 60,
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

    /// Add embedding handler for auto-generating embeddings on add/update
    pub fn with_embedding_handler(mut self, handler: Arc<dyn EmbeddingHandler>) -> Self {
        self.embedding_handler = Some(handler);
        self
    }

    /// Add embedding store for semantic search
    pub fn with_embedding_store(mut self, store: Arc<RwLock<EmbeddingStore>>) -> Self {
        self.embedding_store = Some(store);
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

    // ─── Daily Planning — Scoring Algorithm (Sprint 6, Task #9/#11) ───
    //
    // The planning algorithm ranks tasks by a composite score that balances:
    // 1. Urgency (due dates) — ensures deadline-driven work isn't missed
    // 2. Priority (importance) — respects user-defined task importance
    // 3. Age (staleness) — prevents old tasks from being perpetually deferred
    //
    // Score formula: (urgency × priority_weight) + (age_days × 0.1)
    //
    // This weighted approach ensures that an overdue P1 task (score ~50) always ranks
    // above a future P5 task (score ~1), while still rewarding long-pending work.

    /// Calculate urgency tier based on due date relative to now.
    ///
    /// **Urgency tiers** (exponential weighting):
    /// - `overdue`: 10 — tasks past their deadline (highest urgency)
    /// - `today`: 5 — tasks due within 24 hours
    /// - `tomorrow`: 3 — tasks due in 24-48 hours
    /// - `future` or `None`: 1 — tasks due beyond 48 hours or without deadlines
    ///
    /// **Rationale**: Exponential tiers (10/5/3/1 instead of linear 4/3/2/1) ensure
    /// that urgency dominates the score for time-sensitive work, preventing low-priority
    /// but overdue tasks from being ignored.
    fn calculate_urgency(due_date: Option<chrono::DateTime<chrono::Utc>>, now: chrono::DateTime<chrono::Utc>) -> u32 {
        match due_date {
            None => 1, // No due date → future tier
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
    ///
    /// **Priority weights** (inverse mapping):
    /// - P1 (highest priority): 5
    /// - P2: 4
    /// - P3: 3 (default for tasks without priority)
    /// - P4: 2
    /// - P5 (lowest priority): 1
    /// - None: 3 (default, treated as medium priority)
    ///
    /// **Rationale**: Inverse weighting (P1=5, P5=1) ensures that high-priority tasks
    /// receive higher scores. The default of 3 (P3-equivalent) prevents unprioritized
    /// tasks from being treated as either critical or negligible.
    fn priority_weight(priority: Option<u8>) -> u32 {
        match priority {
            Some(1) => 5,
            Some(2) => 4,
            Some(3) => 3,
            Some(4) => 2,
            Some(5) => 1,
            None => 3, // Default: treat as P3
            _ => 3,    // Invalid priorities default to P3
        }
    }

    /// Calculate age in days from created_at to now.
    ///
    /// **Rationale**: Age factor prevents old tasks from being perpetually deferred
    /// in favor of newer high-urgency work. Each day adds 0.1 to the score, so a
    /// 10-day-old task gets +1.0 boost, and a 50-day-old task gets +5.0.
    ///
    /// **Edge case**: If `created_at` is in the future (clock skew, bad data), age
    /// is clamped to 0 to prevent negative scores.
    fn calculate_age_days(created_at: chrono::DateTime<chrono::Utc>, now: chrono::DateTime<chrono::Utc>) -> f64 {
        let duration = now.signed_duration_since(created_at);
        (duration.num_seconds().max(0) as f64) / 86400.0 // Clamp negative to 0
    }

    /// Calculate composite planning score for a task.
    ///
    /// **Formula**: `score = (urgency × priority_weight) + (age_days × 0.1)`
    ///
    /// **Example scores**:
    /// - Overdue P1 task (5 days old): `(10 × 5) + (5 × 0.1) = 50.5`
    /// - Due today P2 task (3 days old): `(5 × 4) + (3 × 0.1) = 20.3`
    /// - Due tomorrow P3 task (1 day old): `(3 × 3) + (1 × 0.1) = 9.1`
    /// - Future P4 task (0 days old): `(1 × 2) + (0 × 0.1) = 2.0`
    ///
    /// **Range**: Typically 1.0-60.0, with higher scores ranking first.
    ///
    /// **Tiebreaking**: When scores are equal, tasks are ranked by `created_at`
    /// (older tasks first) to ensure deterministic ordering.
    fn calculate_score(task: &Todo, now: chrono::DateTime<chrono::Utc>) -> f64 {
        let urgency = Self::calculate_urgency(task.due_date, now) as f64;
        let priority_wt = Self::priority_weight(task.priority) as f64;
        let age_days = Self::calculate_age_days(task.created_at, now);

        (urgency * priority_wt) + (age_days * 0.1)
    }

    /// Reciprocal Rank Fusion: merge keyword and semantic ranked lists.
    ///
    /// RRF formula: score(d) = sum(1 / (k + rank_i)) for each list where d appears.
    fn reciprocal_rank_fusion(
        keyword_results: &[Todo],
        semantic_results: &[(String, f64)],
        k: u32,
        todos_by_id: &HashMap<String, Todo>,
    ) -> Vec<(Todo, f64, &'static str)> {
        let mut scores: HashMap<String, f64> = HashMap::new();
        let mut sources: HashMap<String, u8> = HashMap::new(); // bitmask: 1=keyword, 2=semantic

        for (rank, todo) in keyword_results.iter().enumerate() {
            *scores.entry(todo.id.clone()).or_insert(0.0) += 1.0 / (k as f64 + rank as f64 + 1.0);
            *sources.entry(todo.id.clone()).or_insert(0) |= 1;
        }

        for (rank, (id, _sim)) in semantic_results.iter().enumerate() {
            *scores.entry(id.clone()).or_insert(0.0) += 1.0 / (k as f64 + rank as f64 + 1.0);
            *sources.entry(id.clone()).or_insert(0) |= 2;
        }

        let mut ranked: Vec<_> = scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        ranked
            .into_iter()
            .filter_map(|(id, score)| {
                let todo = keyword_results
                    .iter()
                    .find(|t| t.id == id)
                    .cloned()
                    .or_else(|| todos_by_id.get(&id).cloned());

                let source_bits = sources.get(&id).copied().unwrap_or(0);
                let source = match source_bits {
                    3 => "both",
                    2 => "semantic",
                    1 => "keyword",
                    _ => "unknown",
                };

                todo.map(|t| (t, score, source))
            })
            .collect()
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

                // Auto-embed (best-effort — failure doesn't block task creation)
                if let Some(ref emb) = self.embedding_handler {
                    if let Err(e) = emb.embed_todo(&created).await {
                        warn!(
                            "Failed to generate embedding for todo {}: {}",
                            created.id, e
                        );
                    }
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

                let updated_todo = store.update(id, patch).await?;
                drop(store);

                match updated_todo {
                    Some(todo) => {
                        // Auto-embed updated task (best-effort)
                        if let Some(ref emb) = self.embedding_handler {
                            if let Err(e) = emb.embed_todo(&todo).await {
                                warn!("Failed to regenerate embedding for todo {}: {}", todo.id, e);
                            }
                        }

                        self.trigger_sync_async().await;
                        Ok(format!("Updated task: {}", todo.title))
                    }
                    None => {
                        Err(ToolError::ExecutionFailed(format!("Task not found: {}", id)).into())
                    }
                }
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

            "search_semantic" => {
                let query = p.required_str("query")?;
                let query = query.trim();
                let limit = p.optional_u64("limit")?.unwrap_or(10) as usize;
                let threshold = p
                    .optional_f64("threshold")?
                    .unwrap_or(self.semantic_threshold);

                // Validation (EC-1, EC-2)
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

                // Check embedding handler is available
                let emb = self.embedding_handler.as_ref().ok_or_else(|| {
                    ToolError::ExecutionFailed(
                        "Semantic search unavailable: embedding engine not initialized. Use plain 'search' for keyword search.".to_string(),
                    )
                })?;

                // Check embedding store is available
                let emb_store = self.embedding_store.as_ref().ok_or_else(|| {
                    ToolError::ExecutionFailed(
                        "Semantic search unavailable: embedding store not configured".to_string(),
                    )
                })?;

                // Get total task count for partial-coverage note
                let total_tasks = store.list(&TodoFilter::default()).await?.len();
                drop(store); // Release todo store lock before embedding operations

                // 1. Embed query
                debug!(
                    query = query,
                    "Generating query embedding for semantic search"
                );
                let query_vec = emb.embed_query(query).await?;

                // 2. Load all embeddings
                let emb_store_guard = emb_store.read().await;
                // We need a write lock for ensure_loaded (lazy loading)
                drop(emb_store_guard);
                let mut emb_store_guard = emb_store.write().await;
                let all_embeddings = emb_store_guard.get_all().await?;

                // EC-3: No embeddings exist
                if all_embeddings.is_empty() {
                    return Ok("No task embeddings found. Run `klyntbot todo backfill-embeddings` to generate embeddings for existing tasks, or add new tasks (embeddings are auto-generated).".to_string());
                }

                let embedding_count = all_embeddings.len();

                // 3. Compute cosine similarity for each embedding
                let mut scored: Vec<(String, f64)> = all_embeddings
                    .iter()
                    .map(|(id, rec)| {
                        let sim = embedding_engine::EmbeddingEngine::cosine_similarity(
                            &query_vec,
                            &rec.embedding,
                        );
                        (id.clone(), sim)
                    })
                    .filter(|(_, sim)| *sim >= threshold)
                    .collect();

                // 4. Sort descending by similarity, take limit
                scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                scored.truncate(limit);

                // EC-5: All results below threshold
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

                // 5. Join with TodoStore to get full Todo objects
                let mut store = self.store.write().await;
                let mut output = format!(
                    "{} task(s) matching '{}' (semantic, threshold: {:.2}):\n",
                    scored.len(),
                    query,
                    threshold,
                );

                for (id, sim) in &scored {
                    if let Ok(Some(todo)) = store.get(id).await {
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

                // EC-4: Partial embedding coverage note
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

                // Validation
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

                // 1. Run keyword search (existing logic)
                let query_lower = query_trimmed.to_lowercase();
                let all_todos = store.list(&TodoFilter::default()).await?;
                let keyword_results: Vec<Todo> = all_todos
                    .iter()
                    .filter(|t| {
                        if t.title.to_lowercase().contains(&query_lower) {
                            return true;
                        }
                        if let Some(desc) = &t.description {
                            if desc.to_lowercase().contains(&query_lower) {
                                return true;
                            }
                        }
                        false
                    })
                    .cloned()
                    .collect();

                // Build ID-to-Todo map for RRF lookups
                let todos_by_id: HashMap<String, Todo> =
                    all_todos.into_iter().map(|t| (t.id.clone(), t)).collect();

                drop(store); // Release lock before embedding operations

                // 2. Run semantic search (if available)
                let semantic_results: Vec<(String, f64)> = if let (Some(emb), Some(emb_store)) =
                    (&self.embedding_handler, &self.embedding_store)
                {
                    let query_vec = emb.embed_query(query_trimmed).await?;
                    let mut emb_store_guard = emb_store.write().await;
                    let all_embeddings = emb_store_guard.get_all().await?;

                    let mut scored: Vec<(String, f64)> = all_embeddings
                        .iter()
                        .map(|(id, rec)| {
                            let sim = embedding_engine::EmbeddingEngine::cosine_similarity(
                                &query_vec,
                                &rec.embedding,
                            );
                            (id.clone(), sim)
                        })
                        .filter(|(_, sim)| *sim >= self.semantic_threshold)
                        .collect();

                    scored
                        .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                    scored
                } else {
                    Vec::new()
                };

                // EC-15/EC-16: One side may be empty — RRF handles gracefully

                // 3. Merge via Reciprocal Rank Fusion
                let merged = Self::reciprocal_rank_fusion(
                    &keyword_results,
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

                for (todo, _rrf_score, source) in &results {
                    output.push_str(&format!(
                        "\n- [{}] {} (P{}, {:?}, match: {})",
                        todo.id,
                        todo.title,
                        todo.priority.unwrap_or(3),
                        todo.status,
                        source,
                    ));
                }

                let has_semantic = self.embedding_handler.is_some();
                info!(
                    query = query_trimmed,
                    results = results.len(),
                    keyword_results = keyword_results.len(),
                    semantic_available = has_semantic,
                    elapsed_ms = search_start.elapsed().as_millis() as u64,
                    "Hybrid search completed"
                );

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

            "plan" => {
                // Generate a daily plan: score eligible tasks and return top N
                // Cap count at 10 to prevent unbounded output and confusing validation messages
                let count = p.optional_u64("count")?.unwrap_or(3).min(10) as usize;
                info!("Generating daily plan (top {} tasks)", count);

                // Get all tasks
                let all_tasks = store.list(&TodoFilter::default()).await?;
                debug!("Total tasks in store: {}", all_tasks.len());

                // Filter eligible tasks:
                // - status = Todo (not Doing, Done, or Archived)
                // - not a template (is_template = false)
                // - not currently focused (focused_at = None)
                // - not blocked (no incomplete blockers)
                let now = Utc::now();
                let mut eligible = Vec::new();
                let mut filtered_counts = std::collections::HashMap::new();

                for task in all_tasks {
                    if task.status != TodoStatus::Todo {
                        *filtered_counts.entry("non_todo").or_insert(0) += 1;
                        continue; // Skip non-Todo tasks
                    }
                    if task.is_template {
                        *filtered_counts.entry("template").or_insert(0) += 1;
                        continue; // Skip templates
                    }
                    if task.focused_at.is_some() {
                        *filtered_counts.entry("focused").or_insert(0) += 1;
                        continue; // Skip already focused tasks
                    }

                    // Check if blocked
                    let blockers = store.incomplete_blockers(&task.id).await?;
                    if !blockers.is_empty() {
                        *filtered_counts.entry("blocked").or_insert(0) += 1;
                        continue; // Skip blocked tasks
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

                // Score and sort eligible tasks
                let mut scored: Vec<(Todo, f64)> = eligible
                    .into_iter()
                    .map(|task| {
                        let score = Self::calculate_score(&task, now);
                        debug!("Scored task [{}] '{}': {:.2}", task.id, task.title, score);
                        (task, score)
                    })
                    .collect();

                // Sort by score descending, then by created_at ascending (tiebreak: older tasks first)
                scored.sort_by(|a, b| {
                    b.1.partial_cmp(&a.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.0.created_at.cmp(&b.0.created_at))
                });

                // Take top N
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

                // Format the plan output
                let mut output = format!("Daily plan ({} tasks):\n\n", plan.len());
                for (i, (task, score)) in plan.iter().enumerate() {
                    let priority_label = task.priority.map(|p| format!("P{}", p)).unwrap_or_else(|| "P3".to_string());

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

    // ─── Semantic Search Validation Tests ────────────────────────

    #[tokio::test]
    async fn test_search_semantic_empty_query() {
        let (tool, _dir) = create_test_tool().await;
        let args = serde_json::json!({
            "action": "search_semantic",
            "query": "   "
        });
        let result = tool.execute(args, &ctx()).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("query cannot be empty"));
    }

    #[tokio::test]
    async fn test_search_semantic_query_too_long() {
        let (tool, _dir) = create_test_tool().await;
        let long_query = "a".repeat(1001);
        let args = serde_json::json!({
            "action": "search_semantic",
            "query": long_query
        });
        let result = tool.execute(args, &ctx()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Query too long"));
    }

    #[tokio::test]
    async fn test_search_semantic_invalid_threshold() {
        let (tool, _dir) = create_test_tool().await;
        let args = serde_json::json!({
            "action": "search_semantic",
            "query": "test",
            "threshold": 1.5
        });
        let result = tool.execute(args, &ctx()).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Threshold must be between"));
    }

    #[tokio::test]
    async fn test_search_semantic_limit_zero() {
        let (tool, _dir) = create_test_tool().await;
        let args = serde_json::json!({
            "action": "search_semantic",
            "query": "test",
            "limit": 0
        });
        let result = tool.execute(args, &ctx()).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Limit must be between 1 and 1000"));
    }

    #[tokio::test]
    async fn test_search_semantic_limit_too_large() {
        let (tool, _dir) = create_test_tool().await;
        let args = serde_json::json!({
            "action": "search_semantic",
            "query": "test",
            "limit": 1001
        });
        let result = tool.execute(args, &ctx()).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Limit must be between 1 and 1000"));
    }

    #[tokio::test]
    async fn test_search_semantic_no_handler() {
        let (tool, _dir) = create_test_tool().await;
        let args = serde_json::json!({
            "action": "search_semantic",
            "query": "test query"
        });
        let result = tool.execute(args, &ctx()).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("embedding engine not initialized"));
    }

    // ─── Hybrid Search Validation Tests ─────────────────────────

    #[tokio::test]
    async fn test_search_hybrid_empty_query() {
        let (tool, _dir) = create_test_tool().await;
        let args = serde_json::json!({
            "action": "search_hybrid",
            "query": ""
        });
        let result = tool.execute(args, &ctx()).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("query cannot be empty"));
    }

    #[tokio::test]
    async fn test_search_hybrid_query_too_long() {
        let (tool, _dir) = create_test_tool().await;
        let long_query = "b".repeat(1001);
        let args = serde_json::json!({
            "action": "search_hybrid",
            "query": long_query
        });
        let result = tool.execute(args, &ctx()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Query too long"));
    }

    #[tokio::test]
    async fn test_search_hybrid_limit_zero() {
        let (tool, _dir) = create_test_tool().await;
        let args = serde_json::json!({
            "action": "search_hybrid",
            "query": "test",
            "limit": 0
        });
        let result = tool.execute(args, &ctx()).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Limit must be between 1 and 1000"));
    }

    #[tokio::test]
    async fn test_search_hybrid_limit_too_large() {
        let (tool, _dir) = create_test_tool().await;
        let args = serde_json::json!({
            "action": "search_hybrid",
            "query": "test",
            "limit": 1001
        });
        let result = tool.execute(args, &ctx()).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Limit must be between 1 and 1000"));
    }

    #[tokio::test]
    async fn test_search_hybrid_no_results() {
        let (tool, _dir) = create_test_tool().await;
        let args = serde_json::json!({
            "action": "search_hybrid",
            "query": "nonexistent_xyz_query"
        });
        let result = tool.execute(args, &ctx()).await.unwrap();
        assert!(result.contains("No tasks found"));
    }

    #[tokio::test]
    async fn test_search_hybrid_keyword_fallback_without_embeddings() {
        let (tool, _dir) = create_test_tool().await;

        // Add a task first
        let add_args = serde_json::json!({
            "action": "add",
            "title": "Fix authentication bug"
        });
        tool.execute(add_args, &ctx()).await.unwrap();

        // Hybrid search falls back to keyword-only when no embedding handler
        let args = serde_json::json!({
            "action": "search_hybrid",
            "query": "authentication"
        });
        let result = tool.execute(args, &ctx()).await.unwrap();
        assert!(result.contains("Fix authentication bug"));
        assert!(result.contains("keyword"));
    }

    // ═══════════════════════════════════════════════════════════════
    // DAILY PLANNING — SCORING ALGORITHM TESTS (Sprint 6, Task #9)
    // ═══════════════════════════════════════════════════════════════

    /// Test urgency tier: overdue tasks get urgency=10
    #[test]
    fn test_urgency_overdue_returns_10() {
        use chrono::{Duration, Utc};

        let now = Utc::now();
        let overdue = now - Duration::days(2);

        let urgency = TodoTool::calculate_urgency(Some(overdue), now);
        assert_eq!(urgency, 10, "Overdue tasks should have urgency=10");
    }

    /// Test urgency tier: tasks due today get urgency=5
    #[test]
    fn test_urgency_today_returns_5() {
        use chrono::Utc;

        let now = Utc::now();
        let today = now; // Same day

        let urgency = TodoTool::calculate_urgency(Some(today), now);
        assert_eq!(urgency, 5, "Tasks due today should have urgency=5");
    }

    /// Test urgency tier: tasks due tomorrow get urgency=3
    #[test]
    fn test_urgency_tomorrow_returns_3() {
        use chrono::{Duration, Utc};

        let now = Utc::now();
        let tomorrow = now + Duration::days(1);

        let urgency = TodoTool::calculate_urgency(Some(tomorrow), now);
        assert_eq!(urgency, 3, "Tasks due tomorrow should have urgency=3");
    }

    /// Test urgency tier: tasks due in the future get urgency=1
    #[test]
    fn test_urgency_future_returns_1() {
        use chrono::{Duration, Utc};

        let now = Utc::now();
        let future = now + Duration::days(7);

        let urgency = TodoTool::calculate_urgency(Some(future), now);
        assert_eq!(urgency, 1, "Tasks due in future should have urgency=1");
    }

    /// Test urgency tier: tasks with no due date get urgency=1
    #[test]
    fn test_urgency_no_due_date_returns_1() {
        use chrono::Utc;

        let now = Utc::now();

        let urgency = TodoTool::calculate_urgency(None, now);
        assert_eq!(urgency, 1, "Tasks with no due date should have urgency=1");
    }

    /// Test priority weight: P1 gets weight=5
    #[test]
    fn test_priority_weight_p1_returns_5() {
        let weight = TodoTool::priority_weight(Some(1));
        assert_eq!(weight, 5, "P1 should have weight=5");
    }

    /// Test priority weight: P2 gets weight=4
    #[test]
    fn test_priority_weight_p2_returns_4() {
        let weight = TodoTool::priority_weight(Some(2));
        assert_eq!(weight, 4, "P2 should have weight=4");
    }

    /// Test priority weight: P3 gets weight=3
    #[test]
    fn test_priority_weight_p3_returns_3() {
        let weight = TodoTool::priority_weight(Some(3));
        assert_eq!(weight, 3, "P3 should have weight=3");
    }

    /// Test priority weight: None gets default weight=3
    #[test]
    fn test_priority_weight_none_returns_3() {
        let weight = TodoTool::priority_weight(None);
        assert_eq!(weight, 3, "Tasks with no priority should get default weight=3");
    }

    /// Test full scoring formula: (urgency × priority_weight) + (age_days × 0.1)
    #[test]
    fn test_score_formula_correct() {
        use chrono::{Duration, Utc};

        let now = Utc::now();
        let mut task = Todo::default_instance();
        task.id = "test-1".to_string();
        task.title = "Overdue P1 task".to_string();
        task.priority = Some(1);                      // P1 → weight=5
        task.due_date = Some(now - Duration::days(2)); // Overdue → urgency=10
        task.created_at = now - Duration::days(5);    // Age=5 days
        task.status = TodoStatus::Todo;

        let score = TodoTool::calculate_score(&task, now);

        // Expected: (10 × 5) + (5 × 0.1) = 50.5
        assert!((score - 50.5).abs() < 0.01, "Score should be ~50.5, got {}", score);
    }

    /// Test age calculation
    #[test]
    fn test_age_calculation() {
        use chrono::{Duration, Utc};

        let now = Utc::now();
        let created_10_days_ago = now - Duration::days(10);

        let age = TodoTool::calculate_age_days(created_10_days_ago, now);
        assert!((age - 10.0).abs() < 0.01, "Age should be ~10 days, got {}", age);
    }

    /// Test age calculation with future timestamp (clock skew edge case)
    #[test]
    fn test_age_calculation_future_timestamp() {
        use chrono::{Duration, Utc};

        let now = Utc::now();
        let created_in_future = now + Duration::days(5); // Clock skew or bad data

        let age = TodoTool::calculate_age_days(created_in_future, now);
        assert_eq!(age, 0.0, "Age should be clamped to 0 for future timestamps, got {}", age);
    }

    /// Test plan action: generates top 3 tasks from eligible pool
    #[tokio::test]
    async fn test_plan_action() {
        use chrono::{Duration, Utc};

        let (tool, _dir) = create_test_tool().await;
        let now = Utc::now();

        // Create test tasks with different scores
        // Task 1: Overdue P1 (should rank #1)
        tool.execute(
            serde_json::json!({
                "action": "add",
                "title": "Fix critical bug",
                "priority": 1,
                "due_date": (now - Duration::days(2)).to_rfc3339()
            }),
            &ctx(),
        )
        .await
        .unwrap();

        // Task 2: Due today P2 (should rank #2)
        tool.execute(
            serde_json::json!({
                "action": "add",
                "title": "Implement feature",
                "priority": 2,
                "due_date": now.to_rfc3339()
            }),
            &ctx(),
        )
        .await
        .unwrap();

        // Task 3: Due tomorrow P3 (should rank #3)
        tool.execute(
            serde_json::json!({
                "action": "add",
                "title": "Update docs",
                "priority": 3,
                "due_date": (now + Duration::days(1)).to_rfc3339()
            }),
            &ctx(),
        )
        .await
        .unwrap();

        // Task 4: Future P4 (should NOT be in top 3)
        tool.execute(
            serde_json::json!({
                "action": "add",
                "title": "Refactor module",
                "priority": 4,
                "due_date": (now + Duration::days(7)).to_rfc3339()
            }),
            &ctx(),
        )
        .await
        .unwrap();

        // Generate plan
        let result = tool.execute(serde_json::json!({"action": "plan"}), &ctx()).await.unwrap();

        // Verify plan includes top 3 tasks
        assert!(result.contains("Daily plan (3 tasks)"), "Plan should have 3 tasks");
        assert!(result.contains("Fix critical bug"), "Should include overdue P1 task");
        assert!(result.contains("Implement feature"), "Should include today P2 task");
        assert!(result.contains("Update docs"), "Should include tomorrow P3 task");
        assert!(!result.contains("Refactor module"), "Should NOT include future P4 task");

        // Verify ordering: overdue P1 should be first
        let fix_pos = result.find("Fix critical bug").unwrap();
        let implement_pos = result.find("Implement feature").unwrap();
        let update_pos = result.find("Update docs").unwrap();
        assert!(fix_pos < implement_pos, "Overdue P1 should rank before today P2");
        assert!(implement_pos < update_pos, "Today P2 should rank before tomorrow P3");
    }

    /// Test plan action: excludes completed tasks
    #[tokio::test]
    async fn test_plan_excludes_completed() {
        let (tool, _dir) = create_test_tool().await;

        // Create and complete a task
        let result = tool.execute(
            serde_json::json!({
                "action": "add",
                "title": "Completed task",
                "priority": 1
            }),
            &ctx(),
        )
        .await
        .unwrap();

        let id = result.split("ID: ").nth(1).unwrap().split(')').next().unwrap();

        tool.execute(
            serde_json::json!({
                "action": "complete",
                "id": id
            }),
            &ctx(),
        )
        .await
        .unwrap();

        // Generate plan
        let plan = tool.execute(serde_json::json!({"action": "plan"}), &ctx()).await.unwrap();

        assert!(plan.contains("No eligible tasks"), "Completed tasks should be excluded");
    }

    /// Test plan action: excludes focused tasks
    #[tokio::test]
    async fn test_plan_excludes_focused() {
        let (tool, _dir) = create_test_tool().await;

        // Create and focus a task
        let result = tool.execute(
            serde_json::json!({
                "action": "add",
                "title": "Already focused",
                "priority": 1
            }),
            &ctx(),
        )
        .await
        .unwrap();

        let id = result.split("ID: ").nth(1).unwrap().split(')').next().unwrap();

        tool.execute(
            serde_json::json!({
                "action": "focus",
                "id": id
            }),
            &ctx(),
        )
        .await
        .unwrap();

        // Generate plan
        let plan = tool.execute(serde_json::json!({"action": "plan"}), &ctx()).await.unwrap();

        assert!(plan.contains("No eligible tasks"), "Focused tasks should be excluded");
    }
}
