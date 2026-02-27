//! TodoTool — feature-todo's primary Tool implementation.

mod actions;

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use crate::calendar_sync::CalendarSyncHandler;
use crate::embedding::EmbeddingHandler;
use crate::enrichment::{EnrichmentFeedbackHandler, EnrichmentHandler};
use crate::types::{Attachment, TimeEntry, Todo};
use common::{Result, ToolError};
use storage::TodoRepo;
use tools_core::{ParamExtractor, RoutingContext, Tool};

/// TodoTool: full todo management with optional enrichment, embedding, and calendar sync.
pub struct TodoTool {
    pub(crate) repo: TodoRepo,
    pub(crate) max_focus_slots: usize,
    pub(crate) focus_deadline_hours: u64,
    pub(crate) timezone: String,
    pub(crate) calendar_handler: Option<Arc<dyn CalendarSyncHandler>>,
    pub(crate) enrichment_handler: Option<Arc<dyn EnrichmentHandler>>,
    pub(crate) embedding_handler: Option<Arc<dyn EmbeddingHandler>>,
    /// LanceDB vector store for semantic search.
    pub(crate) embedding_store: Option<storage::VectorStore>,
    pub(crate) semantic_threshold: f64,
    pub(crate) rrf_k: u32,
    pub(crate) feedback_handler: Option<Arc<dyn EnrichmentFeedbackHandler>>,
}

impl TodoTool {
    /// Create a new TodoTool.
    pub fn new(
        repo: TodoRepo,
        max_focus_slots: usize,
        focus_deadline_hours: u64,
        timezone: String,
    ) -> Self {
        Self {
            repo,
            max_focus_slots,
            focus_deadline_hours,
            timezone,
            calendar_handler: None,
            enrichment_handler: None,
            embedding_handler: None,
            embedding_store: None,
            semantic_threshold: 0.5,
            rrf_k: 60,
            feedback_handler: None,
        }
    }

    /// Attach a calendar sync handler.
    pub fn with_calendar_handler(mut self, handler: Arc<dyn CalendarSyncHandler>) -> Self {
        self.calendar_handler = Some(handler);
        self
    }

    /// Attach an enrichment handler.
    pub fn with_enrichment_handler(mut self, handler: Arc<dyn EnrichmentHandler>) -> Self {
        self.enrichment_handler = Some(handler);
        self
    }

    /// Attach an embedding handler for auto-embedding on add/update.
    pub fn with_embedding_handler(mut self, handler: Arc<dyn EmbeddingHandler>) -> Self {
        self.embedding_handler = Some(handler);
        self
    }

    /// Attach a vector store for LanceDB semantic search.
    pub fn with_embedding_store(mut self, store: storage::VectorStore) -> Self {
        self.embedding_store = Some(store);
        self
    }

    /// Set semantic search configuration.
    pub fn with_search_config(mut self, threshold: f64, rrf_k: u32) -> Self {
        self.semantic_threshold = threshold;
        self.rrf_k = rrf_k;
        self
    }

    // ─── Scoring helpers ───────────────────────────────────────────

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
                    10
                } else if due_date == now_date {
                    5
                } else if due_date == now_date + chrono::Duration::days(1) {
                    3
                } else {
                    1
                }
            }
        }
    }

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

    pub(crate) fn calculate_age_days(
        created_at: chrono::DateTime<chrono::Utc>,
        now: chrono::DateTime<chrono::Utc>,
    ) -> f64 {
        let duration = now.signed_duration_since(created_at);
        (duration.num_seconds().max(0) as f64) / 86400.0
    }

    pub(crate) fn calculate_score(task: &Todo, now: chrono::DateTime<chrono::Utc>) -> f64 {
        let urgency = Self::calculate_urgency(task.due_date, now) as f64;
        let priority_wt = Self::priority_weight(task.priority) as f64;
        let age_days = Self::calculate_age_days(task.created_at, now);
        (urgency * priority_wt) + (age_days * 0.1)
    }

    // ─── Full todo loading ─────────────────────────────────────────

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
                    "enum": [
                        "add", "list", "update", "complete", "delete", "show", "summary",
                        "focus", "unfocus", "add_subtask", "move", "attach", "detach",
                        "log_time", "tree", "search", "search_semantic", "search_hybrid",
                        "report", "add_dependency", "remove_dependency",
                        "recur", "list_recurring", "delete_recurring", "enrich", "plan",
                        "execute"
                    ],
                    "description": "Action to perform"
                },
                "id": { "type": "string", "description": "Task ID" },
                "title": { "type": "string", "description": "Task title" },
                "description": { "type": "string", "description": "Task description" },
                "priority": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 5,
                    "description": "Priority 1-5"
                },
                "due_date": {
                    "type": "string",
                    "description": "Due date (RFC3339, 'YYYY-MM-DD HH:MM', or 'YYYY-MM-DD')"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tags"
                },
                "status": {
                    "type": "string",
                    "enum": ["todo", "doing", "done", "archived"]
                },
                "priority_min": { "type": "integer", "description": "Min priority filter (list)" },
                "tag": { "type": "string", "description": "Tag filter (list)" },
                "limit": { "type": "integer", "description": "Max results" },
                "parent_id": { "type": "string", "description": "Parent task ID" },
                "project_id": { "type": "string", "description": "Project ID" },
                "new_parent_id": { "type": "string", "description": "New parent task ID (move)" },
                "new_project_id": { "type": "string", "description": "New project ID (move)" },
                "attachment_type": {
                    "type": "string",
                    "enum": ["file", "url", "note"],
                    "description": "Attachment type"
                },
                "value": { "type": "string", "description": "Attachment value" },
                "attachment_title": { "type": "string", "description": "Attachment title" },
                "attachment_id": { "type": "string", "description": "Attachment ID (detach)" },
                "duration_minutes": { "type": "integer", "description": "Duration (log_time)" },
                "note": { "type": "string", "description": "Time entry note" },
                "query": { "type": "string", "description": "Search query" },
                "threshold": {
                    "type": "number",
                    "description": "Similarity threshold for semantic search (0.0-1.0)"
                },
                "period": {
                    "type": "string",
                    "enum": ["week", "month"],
                    "description": "Time period for report"
                },
                "rule": {
                    "type": "string",
                    "description": "RRULE string e.g. 'FREQ=DAILY;BYHOUR=9'"
                },
                "template_id": {
                    "type": "string",
                    "description": "Recurring task template ID (delete_recurring)"
                },
                "count": {
                    "type": "integer",
                    "description": "Number of tasks to include in plan (default: 3)"
                }
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
            "execute" => self.handle_execute(&p).await,
            _ => Err(ToolError::InvalidParams(format!("Unknown action: {}", action)).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::{Duration, Utc};

    #[test]
    fn test_urgency_overdue() {
        let now = Utc::now();
        assert_eq!(
            TodoTool::calculate_urgency(Some(now - Duration::days(2)), now),
            10
        );
    }

    #[test]
    fn test_urgency_today() {
        let now = Utc::now();
        assert_eq!(TodoTool::calculate_urgency(Some(now), now), 5);
    }

    #[test]
    fn test_urgency_tomorrow() {
        let now = Utc::now();
        assert_eq!(
            TodoTool::calculate_urgency(Some(now + Duration::days(1)), now),
            3
        );
    }

    #[test]
    fn test_urgency_future() {
        let now = Utc::now();
        assert_eq!(
            TodoTool::calculate_urgency(Some(now + Duration::days(7)), now),
            1
        );
    }

    #[test]
    fn test_urgency_no_due_date() {
        let now = Utc::now();
        assert_eq!(TodoTool::calculate_urgency(None, now), 1);
    }

    #[test]
    fn test_priority_weight_p1() {
        assert_eq!(TodoTool::priority_weight(Some(1)), 5);
    }

    #[test]
    fn test_priority_weight_none() {
        assert_eq!(TodoTool::priority_weight(None), 3);
    }

    #[test]
    fn test_score_formula() {
        let now = Utc::now();
        let mut task = Todo::default_instance();
        task.priority = Some(1);
        task.due_date = Some(now - Duration::days(2));
        task.created_at = now - Duration::days(5);
        let score = TodoTool::calculate_score(&task, now);
        // urgency=10, priority_wt=5, age=5 → 10*5 + 5*0.1 = 50.5
        assert!(
            (score - 50.5).abs() < 0.01,
            "Score should be ~50.5, got {}",
            score
        );
    }
}
