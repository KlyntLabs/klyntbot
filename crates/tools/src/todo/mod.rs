//! TodoTool - Tool interface for todo system
//!
//! Provides 26 actions for complete todo management through the Tool trait.
//! Action handlers are split into sub-modules under `actions/`.

mod actions;

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use super::{RoutingContext, Tool};
use crate::calendar_tool::CalendarHandler;
use crate::embedding_engine::EmbeddingHandler;
use crate::enrichment::EnrichmentHandler;
use crate::params::ParamExtractor;
use crate::todo_types::{Attachment, TimeEntry, Todo};
use common::{Result, ToolError};
use tracing::warn;

/// TodoTool with config-driven focus values (ADR-008)
pub struct TodoTool {
    pub(crate) repo: storage::TodoRepo,
    pub(crate) max_focus_slots: usize,
    pub(crate) focus_deadline_hours: u64,
    pub(crate) calendar_handler: Option<Arc<dyn CalendarHandler>>,
    pub(crate) enrichment_handler: Option<Arc<dyn EnrichmentHandler>>,
    pub(crate) embedding_handler: Option<Arc<dyn EmbeddingHandler>>,
    pub(crate) embedding_repo: Option<storage::EmbeddingRepo>,
    /// Default cosine similarity threshold for semantic search (from config)
    pub(crate) semantic_threshold: f64,
    /// RRF k parameter for hybrid search (from config)
    pub(crate) rrf_k: u32,
    pub(crate) timezone: String,
    /// Learning feedback handler for reporting enrichment outcomes
    pub(crate) feedback_handler: Option<Arc<dyn crate::EnrichmentFeedbackHandler>>,
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

    // ─── Internal helpers ─────────────────────────────────────

    /// Trigger immediate calendar sync (best-effort, don't fail if sync fails)
    pub(crate) async fn trigger_sync_async(&self) {
        if let Some(handler) = &self.calendar_handler {
            if let Err(e) = handler.sync_calendar().await {
                warn!("Immediate calendar sync failed: {}", e);
            }
        }
    }

    /// Push a single task to calendar (best-effort).
    pub(crate) async fn push_task_to_calendar(&self, task_id: &str) {
        if let Some(handler) = &self.calendar_handler {
            if let Err(e) = handler.push_single_task(task_id).await {
                warn!("Calendar push for task {} failed: {}", task_id, e);
            }
        }
    }

    /// Remove a calendar event by UID (best-effort).
    pub(crate) async fn remove_event_from_calendar(&self, uid: &str) {
        if let Some(handler) = &self.calendar_handler {
            if let Err(e) = handler.remove_single_event(uid).await {
                warn!("Calendar event removal for {} failed: {}", uid, e);
            }
        }
    }

    /// Record enrichment feedback for all suggestion fields (priority, duration, due date).
    /// When `accepted_override` is `Some(true)`, all fields are marked accepted (manual enrich).
    /// When `None`, each field's confidence is checked against 0.7 threshold (auto-enrich).
    pub(crate) async fn record_enrichment_feedback(
        &self,
        task_id: &str,
        result: &crate::enrichment::EnrichmentResult,
        accepted_override: Option<bool>,
    ) {
        let Some(fb) = &self.feedback_handler else {
            return;
        };
        let now = chrono::Utc::now();

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
    pub(crate) fn calculate_urgency(
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
    pub(crate) fn priority_weight(priority: Option<u8>) -> u32 {
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
    pub(crate) fn calculate_age_days(
        created_at: chrono::DateTime<chrono::Utc>,
        now: chrono::DateTime<chrono::Utc>,
    ) -> f64 {
        let duration = now.signed_duration_since(created_at);
        (duration.num_seconds().max(0) as f64) / 86400.0
    }

    /// Calculate composite planning score for a task.
    pub(crate) fn calculate_score(task: &Todo, now: chrono::DateTime<chrono::Utc>) -> f64 {
        let urgency = Self::calculate_urgency(task.due_date, now) as f64;
        let priority_wt = Self::priority_weight(task.priority) as f64;
        let age_days = Self::calculate_age_days(task.created_at, now);
        (urgency * priority_wt) + (age_days * 0.1)
    }

    // ─── Helper: convert row to domain Todo with deps/attachments/time_entries ───

    /// Load a full domain Todo from a row, including related data.
    pub(crate) async fn load_full_todo(&self, row: storage::TodoRow) -> Result<Todo> {
        let id = row.id.clone();
        let mut todo = Todo::from(row);

        let att_rows = self.repo.list_attachments(&id).await?;
        todo.attachments = att_rows.into_iter().map(Attachment::from).collect();

        let te_rows = self.repo.list_time_entries(&id).await?;
        todo.time_entries = te_rows.into_iter().map(TimeEntry::from).collect();

        let blockers = self.repo.get_blockers(&id).await?;
        todo.blocked_by = blockers.into_iter().map(|r| r.id).collect();
        let blocking = self.repo.get_blocking(&id).await?;
        todo.blocks = blocking.into_iter().map(|r| r.id).collect();

        Ok(todo)
    }

    /// Load a domain Todo by ID, or return None.
    pub(crate) async fn get_full_todo(&self, id: &str) -> Result<Option<Todo>> {
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
            "add" => self.handle_add(&p).await,
            "list" => self.handle_list(&p).await,
            "update" => self.handle_update(&p).await,
            "complete" => self.handle_complete(&p).await,
            "delete" => self.handle_delete(&p).await,
            "enrich" => self.handle_enrich(&p).await,
            "show" => self.handle_show(&p).await,
            "summary" => self.handle_summary().await,
            "focus" => self.handle_focus(&p).await,
            "unfocus" => self.handle_unfocus(&p).await,
            "add_subtask" => self.handle_add_subtask(&p).await,
            "move" => self.handle_move(&p).await,
            "attach" => self.handle_attach(&p).await,
            "detach" => self.handle_detach(&p).await,
            "log_time" => self.handle_log_time(&p).await,
            "tree" => self.handle_tree().await,
            "search" => self.handle_search(&p).await,
            "search_semantic" => self.handle_search_semantic(&p).await,
            "search_hybrid" => self.handle_search_hybrid(&p).await,
            "report" => self.handle_report(&p).await,
            "add_dependency" => self.handle_add_dependency(&p).await,
            "remove_dependency" => self.handle_remove_dependency(&p).await,
            "recur" => self.handle_recur(&p).await,
            "list_recurring" => self.handle_list_recurring().await,
            "delete_recurring" => self.handle_delete_recurring(&p).await,
            "plan" => self.handle_plan(&p).await,
            _ => Err(ToolError::InvalidParams(format!("Unknown action: {}", action)).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::todo_types::TodoStatus;

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
