//! TaskTool — feature-tasks' primary Tool implementation.

mod actions;

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use bus::DomainEventBus;

use crate::config::TasksConfig;
use crate::handlers::{
    DayPlanningHandler, DecompositionHandler, EmbeddingHandler, EnrichmentHandler, ForecastHandler,
    ProactiveHandler, SuggestionApplier, TaskExecutionHandler,
};
use crate::types::{Attachment, Task, TimeEntry};
use common::{Result, ToolError};
use storage::TaskRepo;
use tools_core::{ParamExtractor, ProgressHandler, RoutingContext, Tool};

/// TaskTool: agentic task management with enrichment, embedding, and planning.
pub struct TaskTool {
    pub(crate) repo: TaskRepo,
    pub(crate) area_repo: Option<storage::AreaRepo>,
    pub(crate) max_focus_slots: usize,
    pub(crate) focus_deadline_hours: u64,
    pub(crate) timezone: String,
    pub(crate) enrichment_handler: Option<Arc<dyn EnrichmentHandler>>,
    pub(crate) embedding_handler: Option<Arc<dyn EmbeddingHandler>>,
    /// LanceDB vector store for semantic search.
    pub(crate) embedding_store: Option<storage::VectorStore>,
    pub(crate) semantic_threshold: f64,
    pub(crate) rrf_k: u32,
    /// Confidence threshold for auto-applying enrichment suggestions (0.0-1.0).
    pub(crate) enrichment_threshold: f64,
    /// Optional progress handler for cascading KR progress on complete.
    pub(crate) progress_handler: Option<Arc<dyn ProgressHandler>>,
    /// Optional domain event bus for cross-feature communication.
    pub(crate) domain_bus: Option<Arc<DomainEventBus>>,
    /// Feature configuration.
    pub(crate) config: TasksConfig,
    /// Optional decomposition handler (LLM-powered subtask generation).
    pub(crate) decomposition_handler: Option<Arc<dyn DecompositionHandler>>,
    /// Optional execution handler (agentic task execution).
    pub(crate) execution_handler: Option<Arc<dyn TaskExecutionHandler>>,
    /// Optional day planning handler (LLM-powered daily planning).
    pub(crate) planning_handler: Option<Arc<dyn DayPlanningHandler>>,
    /// Optional proactive suggestion handler.
    pub(crate) proactive_handler: Option<Arc<dyn ProactiveHandler>>,
    /// Optional suggestion applier for executing accepted suggestions.
    pub(crate) suggestion_applier: Option<Arc<dyn SuggestionApplier>>,
    /// Optional forecast handler for estimation accuracy and project forecasting.
    pub(crate) forecast_handler: Option<Arc<dyn ForecastHandler>>,
}

impl TaskTool {
    /// Create a new TaskTool.
    pub fn new(
        repo: TaskRepo,
        max_focus_slots: usize,
        focus_deadline_hours: u64,
        timezone: String,
    ) -> Self {
        Self {
            repo,
            area_repo: None,
            max_focus_slots,
            focus_deadline_hours,
            timezone,
            enrichment_handler: None,
            embedding_handler: None,
            embedding_store: None,
            semantic_threshold: 0.5,
            rrf_k: 60,
            enrichment_threshold: 0.70,
            progress_handler: None,
            domain_bus: None,
            config: TasksConfig::default(),
            decomposition_handler: None,
            execution_handler: None,
            planning_handler: None,
            proactive_handler: None,
            suggestion_applier: None,
            forecast_handler: None,
        }
    }

    /// Attach an area repo for auto-resolving area_id on create.
    pub fn with_area_repo(mut self, repo: storage::AreaRepo) -> Self {
        self.area_repo = Some(repo);
        self
    }

    /// Attach an enrichment handler.
    pub fn with_enrichment_handler(mut self, handler: Arc<dyn EnrichmentHandler>) -> Self {
        self.enrichment_handler = Some(handler);
        self
    }

    /// Attach an embedding handler for auto-embedding on create/update.
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

    /// Set the enrichment auto-apply confidence threshold.
    pub fn with_enrichment_threshold(mut self, threshold: f64) -> Self {
        self.enrichment_threshold = threshold;
        self
    }

    /// Attach a progress handler for cascading KR progress on task complete.
    pub fn with_progress_handler(mut self, handler: Arc<dyn ProgressHandler>) -> Self {
        self.progress_handler = Some(handler);
        self
    }

    /// Attach a domain event bus for cross-feature communication.
    pub fn with_domain_bus(mut self, bus: Arc<DomainEventBus>) -> Self {
        self.domain_bus = Some(bus);
        self
    }

    /// Set the full feature configuration.
    pub fn with_config(mut self, config: TasksConfig) -> Self {
        self.max_focus_slots = config.max_focus_slots;
        self.focus_deadline_hours = config.focus_deadline_hours;
        self.timezone = config.timezone.clone();
        self.enrichment_threshold = config.enrichment.auto_apply_threshold;
        self.semantic_threshold = config.search.semantic_threshold;
        self.rrf_k = config.search.rrf_k;
        self.config = config;
        self
    }

    /// Attach a decomposition handler for AI-powered subtask generation.
    pub fn with_decomposition_handler(mut self, handler: Arc<dyn DecompositionHandler>) -> Self {
        self.decomposition_handler = Some(handler);
        self
    }

    /// Attach an execution handler for agentic task execution.
    pub fn with_execution_handler(mut self, handler: Arc<dyn TaskExecutionHandler>) -> Self {
        self.execution_handler = Some(handler);
        self
    }

    /// Attach a day planning handler for LLM-powered daily planning.
    pub fn with_planning_handler(mut self, handler: Arc<dyn DayPlanningHandler>) -> Self {
        self.planning_handler = Some(handler);
        self
    }

    /// Attach a proactive suggestion handler.
    pub fn with_proactive_handler(mut self, handler: Arc<dyn ProactiveHandler>) -> Self {
        self.proactive_handler = Some(handler);
        self
    }

    /// Attach a suggestion applier for executing accepted suggestions.
    pub fn with_suggestion_applier(mut self, applier: Arc<dyn SuggestionApplier>) -> Self {
        self.suggestion_applier = Some(applier);
        self
    }

    /// Attach a forecast handler for estimation accuracy and project forecasting.
    pub fn with_forecast_handler(mut self, handler: Arc<dyn ForecastHandler>) -> Self {
        self.forecast_handler = Some(handler);
        self
    }

    // ─── Scoring helpers ───────────────────────────────────────────

    pub(crate) fn calculate_score(task: &Task, now: chrono::DateTime<chrono::Utc>) -> f64 {
        crate::scoring::calculate_score(task, now)
    }

    // ─── Full task loading ──────────────────────────────────────────

    pub(crate) async fn load_full_task(&self, row: storage::TaskRow) -> Result<Task> {
        let id = row.id.clone();
        let mut task = Task::from(row);

        let (att_rows, te_rows, blockers, blocking) = tokio::try_join!(
            self.repo.list_attachments(&id),
            self.repo.list_time_entries(&id),
            self.repo.get_blockers(&id),
            self.repo.get_blocking(&id),
        )?;

        task.attachments = att_rows.into_iter().map(Attachment::from).collect();
        task.time_entries = te_rows.into_iter().map(TimeEntry::from).collect();
        task.blocked_by = blockers.into_iter().map(|r| r.id).collect();
        task.blocks = blocking.into_iter().map(|r| r.id).collect();

        Ok(task)
    }

    pub(crate) async fn get_full_task(&self, id: &str) -> Result<Option<Task>> {
        match self.repo.get(id).await? {
            Some(row) => Ok(Some(self.load_full_task(row).await?)),
            None => Ok(None),
        }
    }

    /// Load a task by ID or return a not-found error.
    pub(crate) async fn require_task(&self, id: &str) -> Result<Task> {
        self.get_full_task(id)
            .await?
            .ok_or_else(|| ToolError::ExecutionFailed(format!("Task {id} not found")).into())
    }
}

#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &str {
        "tasks"
    }

    fn description(&self) -> &str {
        "Manage individual to-do items and action items. Create, complete, update, delete, list, and search tasks. Supports priorities, dependencies, time logging, recurring tasks, daily planning, and AI-assisted decomposition. NOT for project containers or OKR objectives."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "create", "update", "complete", "reopen", "delete",
                        "show", "list", "summary", "tree",
                        "search",
                        "focus", "unfocus", "log_time",
                        "add_dep", "remove_dep",
                        "batch",
                        "recur", "list_recurring", "delete_recurring",
                        "plan_day",
                        "decompose", "execute", "cancel_execution",
                        "suggest", "apply_suggestion", "dismiss_suggestion", "list_suggestions",
                        "forecast_task", "forecast_project", "accuracy_report"
                    ],
                    "description": "Action to perform"
                },
                "id": { "type": "string", "description": "Task ID" },
                "title": { "type": "string", "description": "Task title" },
                "description": { "type": "string", "description": "Task description" },
                "area_id": { "type": "string", "description": "Area ID (required for create)" },
                "key_result_id": { "type": "string", "description": "Key result ID (optional, links task to OKR)" },
                "objective_id": { "type": "string", "description": "Objective ID" },
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
                    "enum": ["todo", "doing", "done", "someday"],
                    "description": "Task status"
                },
                "task_type": {
                    "type": "string",
                    "enum": ["manual", "agentic", "hybrid"],
                    "description": "Task type (default: manual)"
                },
                "acceptance_criteria": {
                    "type": "string",
                    "description": "Acceptance criteria for the task"
                },
                "energy_level": {
                    "type": "string",
                    "enum": ["low", "medium", "high", "deep"],
                    "description": "Energy level required"
                },
                "agent_config": {
                    "type": "string",
                    "description": "JSON agent configuration string"
                },
                "estimated_minutes": {
                    "type": "integer",
                    "description": "Estimated duration in minutes"
                },
                "priority_min": { "type": "integer", "description": "Min priority filter (list)" },
                "tag": { "type": "string", "description": "Tag filter (list)" },
                "limit": { "type": "integer", "description": "Max results" },
                "parent_id": { "type": "string", "description": "Parent task ID" },
                "project_id": { "type": "string", "description": "Project ID" },
                "duration_minutes": { "type": "integer", "description": "Duration (log_time)" },
                "note": { "type": "string", "description": "Time entry note" },
                "query": { "type": "string", "description": "Search query" },
                "threshold": {
                    "type": "number",
                    "description": "Similarity threshold for semantic search (0.0-1.0)"
                },
                "task_id": { "type": "string", "description": "Task ID (for dependencies)" },
                "blocked_by": { "type": "string", "description": "Blocker task ID (for dependencies)" },
                "dep_type": {
                    "type": "string",
                    "enum": ["blocks", "depends_on", "related"],
                    "description": "Dependency type (default: blocks)"
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
                },
                "execution_id": { "type": "string", "description": "Execution ID (for cancel_execution)" },
                "suggestion_id": { "type": "string", "description": "Suggestion ID (for apply_suggestion/dismiss_suggestion)" },
                "agent_profile": { "type": "string", "description": "Agent profile for execution" },
                "max_cost": { "type": "number", "description": "Max cost in USD for execution" },
                "max_iterations": { "type": "integer", "description": "Max LLM iterations for execution" },
                "timeout_secs": { "type": "integer", "description": "Timeout in seconds for execution" },
                "require_approval": { "type": "boolean", "description": "Require approval before execution" },
                "unassigned": {
                    "type": "boolean",
                    "description": "Filter for tasks not assigned to any project or area (list)"
                },
                "due_after": {
                    "type": "string",
                    "description": "Filter tasks due after this date (RFC3339 or YYYY-MM-DD)"
                },
                "due_before": {
                    "type": "string",
                    "description": "Filter tasks due before this date (RFC3339 or YYYY-MM-DD)"
                },
                "status_label_id": { "type": "string", "description": "Status label ID (for update)" },
                "group_id": { "type": "string", "description": "Group ID (for update)" },
                "scheduled_start": { "type": "string", "description": "Scheduled start datetime (for update)" },
                "scheduled_end": { "type": "string", "description": "Scheduled end datetime (for update)" },
                "operations": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "op": {
                                "type": "string",
                                "enum": ["complete", "delete", "tag", "reorder"],
                                "description": "Batch operation type"
                            },
                            "id": { "type": "string", "description": "Task ID" },
                            "tags": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Tags to add (tag op)"
                            },
                            "position": {
                                "type": "integer",
                                "description": "New position (reorder op)"
                            }
                        }
                    },
                    "description": "Batch operations (batch action)"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String> {
        let p = ParamExtractor::new(&args);
        let action = p.required_str("action")?;

        match action {
            "create" => self.handle_create(&p, ctx).await,
            "update" => self.handle_update(&p).await,
            "complete" => self.handle_complete(&p).await,
            "reopen" => self.handle_reopen(&p).await,
            "delete" => self.handle_delete(&p).await,
            "show" => self.handle_show(&p).await,
            "list" => self.handle_list(&p).await,
            "summary" => self.handle_summary().await,
            "tree" => self.handle_tree().await,
            "search" => self.handle_search(&p).await,
            "focus" => self.handle_focus(&p).await,
            "unfocus" => self.handle_unfocus(&p).await,
            "log_time" => self.handle_log_time(&p).await,
            "add_dep" => self.handle_add_dep(&p).await,
            "remove_dep" => self.handle_remove_dep(&p).await,
            "batch" => self.handle_batch(&p).await,
            "recur" => self.handle_recur(&p).await,
            "list_recurring" => self.handle_list_recurring().await,
            "delete_recurring" => self.handle_delete_recurring(&p).await,
            "plan_day" => self.handle_plan_day(&p).await,
            "decompose" => self.handle_decompose(&p).await,
            "execute" => self.handle_execute(&p).await,
            "cancel_execution" => self.handle_cancel_execution(&p).await,
            "suggest" => self.handle_suggest(&p).await,
            "apply_suggestion" => self.handle_apply_suggestion(&p).await,
            "dismiss_suggestion" => self.handle_dismiss_suggestion(&p).await,
            "list_suggestions" => self.handle_list_suggestions(&p).await,
            "forecast_task" => self.handle_forecast_task(&p).await,
            "forecast_project" => self.handle_forecast_project(&p).await,
            "accuracy_report" => self.handle_accuracy_report(&p).await,
            _ => Err(ToolError::InvalidParams(format!("Unknown action: {action}")).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scoring;

    use chrono::{Duration, Utc};

    #[test]
    fn test_urgency_overdue() {
        let now = Utc::now();
        assert_eq!(
            scoring::calculate_urgency(Some(now - Duration::days(2)), now),
            10
        );
    }

    #[test]
    fn test_urgency_today() {
        let now = Utc::now();
        assert_eq!(scoring::calculate_urgency(Some(now), now), 5);
    }

    #[test]
    fn test_urgency_tomorrow() {
        let now = Utc::now();
        assert_eq!(
            scoring::calculate_urgency(Some(now + Duration::days(1)), now),
            3
        );
    }

    #[test]
    fn test_urgency_future() {
        let now = Utc::now();
        assert_eq!(
            scoring::calculate_urgency(Some(now + Duration::days(7)), now),
            1
        );
    }

    #[test]
    fn test_urgency_no_due_date() {
        let now = Utc::now();
        assert_eq!(scoring::calculate_urgency(None, now), 1);
    }

    #[test]
    fn test_priority_weight_p1() {
        assert_eq!(scoring::priority_weight(Some(1)), 5);
    }

    #[test]
    fn test_priority_weight_none() {
        assert_eq!(scoring::priority_weight(None), 3);
    }

    #[test]
    fn test_score_formula() {
        let now = Utc::now();
        let mut task = Task::default_instance();
        task.priority = Some(1);
        task.due_date = Some(now - Duration::days(2));
        task.created_at = now - Duration::days(5);
        let score = TaskTool::calculate_score(&task, now);
        // urgency=10, priority_wt=5, age=5 -> 10*5 + 5*0.1 = 50.5
        assert!(
            (score - 50.5).abs() < 0.01,
            "Score should be ~50.5, got {}",
            score
        );
    }

    // ─── Integration tests ──────────────────────────────────────────

    /// Create task tables for integration tests (without FK constraints
    /// to external tables like projects/key_results).
    async fn create_test_tables(db: &sqlx::SqlitePool) {
        // Relaxed schema: removes FK references to projects, key_results, etc.
        for sql in &[
            r#"CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY, title TEXT NOT NULL, description TEXT,
                area_id TEXT NOT NULL REFERENCES areas(id) ON DELETE CASCADE,
                project_id TEXT, key_result_id TEXT, objective_id TEXT,
                parent_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
                status_label_id TEXT, group_id TEXT,
                priority INTEGER, position INTEGER NOT NULL DEFAULT 0,
                due_date INTEGER, tags TEXT NOT NULL DEFAULT '[]',
                status TEXT NOT NULL DEFAULT 'todo',
                task_type TEXT NOT NULL DEFAULT 'manual',
                focused_at INTEGER, focus_deadline INTEGER,
                focus_expired_count INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
                updated_at INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
                completed_at INTEGER, completed INTEGER NOT NULL DEFAULT 0,
                total_tracked_secs INTEGER NOT NULL DEFAULT 0,
                estimated_minutes INTEGER, actual_minutes INTEGER,
                calendar_event_uid TEXT, last_reminded_at INTEGER,
                recurrence_rule TEXT, recurrence_parent_id TEXT,
                is_template INTEGER NOT NULL DEFAULT 0, next_instance_date INTEGER,
                acceptance_criteria TEXT, agent_config TEXT,
                execution_state TEXT NOT NULL DEFAULT 'idle',
                spawned_execution_id TEXT, context_snapshot TEXT,
                energy_level TEXT DEFAULT 'medium',
                estimated_focus_blocks INTEGER, complexity_score INTEGER,
                scheduled_start INTEGER, scheduled_end INTEGER
            )"#,
            r#"CREATE TABLE IF NOT EXISTS task_activity (
                id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                activity_type TEXT NOT NULL, field_changed TEXT, old_value TEXT, new_value TEXT,
                actor_type TEXT NOT NULL DEFAULT 'user', actor_id TEXT, summary TEXT,
                created_at INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000)
            )"#,
            r#"CREATE TABLE IF NOT EXISTS task_attachments (
                id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                attachment_type TEXT NOT NULL, value TEXT NOT NULL, title TEXT,
                tags TEXT NOT NULL DEFAULT '[]', source TEXT NOT NULL DEFAULT 'user',
                created_at INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000)
            )"#,
            r#"CREATE TABLE IF NOT EXISTS task_time_entries (
                id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                source TEXT NOT NULL DEFAULT 'focus', started_at INTEGER NOT NULL, ended_at INTEGER,
                duration_secs INTEGER, energy_level TEXT, note TEXT
            )"#,
            r#"CREATE TABLE IF NOT EXISTS task_dependencies (
                task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                blocker_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                dep_type TEXT NOT NULL DEFAULT 'blocks',
                PRIMARY KEY (task_id, blocker_id),
                CHECK (task_id != blocker_id)
            )"#,
            r#"CREATE TABLE IF NOT EXISTS task_executions (
                id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                status TEXT NOT NULL DEFAULT 'pending', agent_profile TEXT,
                started_at INTEGER, completed_at INTEGER, duration_secs INTEGER,
                tokens_used INTEGER, cost_usd REAL, input_context TEXT,
                output_summary TEXT, error_message TEXT, artifacts TEXT, metrics TEXT,
                retry_count INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000)
            )"#,
            r#"CREATE TABLE IF NOT EXISTS task_suggestions (
                id TEXT PRIMARY KEY, task_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
                suggestion_type TEXT NOT NULL, title TEXT NOT NULL, description TEXT,
                confidence REAL NOT NULL DEFAULT 0.0, action_payload TEXT,
                status TEXT NOT NULL DEFAULT 'pending', trigger TEXT,
                created_at INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
                resolved_at INTEGER
            )"#,
            r#"CREATE TABLE IF NOT EXISTS task_estimation_history (
                id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                estimated_minutes INTEGER NOT NULL, actual_minutes INTEGER NOT NULL,
                deviation_pct REAL NOT NULL DEFAULT 0.0, complexity_score INTEGER,
                energy_level TEXT, tags TEXT NOT NULL DEFAULT '[]',
                project_id TEXT,
                completed_at INTEGER NOT NULL
            )"#,
            r#"CREATE TABLE IF NOT EXISTS task_decompositions (
                id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                plan TEXT NOT NULL, confidence REAL NOT NULL DEFAULT 0.0,
                status TEXT NOT NULL DEFAULT 'pending', reasoning TEXT,
                created_at INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
                applied_at INTEGER
            )"#,
            "INSERT OR IGNORE INTO areas (id, name, color, status) VALUES ('test-area', 'Test Area', '#000', 'active')",
        ] {
            sqlx::query(sql).execute(db).await.unwrap();
        }
    }

    async fn make_tool() -> TaskTool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let db = pool.inner().clone();
        create_test_tables(&db).await;
        let repo = storage::TaskRepo::new(db);
        TaskTool::new(repo, 3, 8, "UTC".to_string())
    }

    fn test_ctx() -> RoutingContext {
        RoutingContext::new(common::ChannelName::new("cli"), common::ChatId::new("test"))
    }

    #[tokio::test]
    async fn test_create_task() {
        let tool = make_tool().await;
        let ctx = test_ctx();
        let args = serde_json::json!({
            "action": "create",
            "title": "Write tests",
            "area_id": "test-area",
            "priority": 2,
            "task_type": "manual",
            "energy_level": "high"
        });

        let result = tool.execute(args, &ctx).await.unwrap();
        assert!(result.contains("Task created: Write tests"));
        assert!(result.contains("type: manual"));

        // Verify task exists via list
        let list_args = serde_json::json!({ "action": "list" });
        let list_result = tool.execute(list_args, &ctx).await.unwrap();
        assert!(list_result.contains("Write tests"));
    }

    #[tokio::test]
    async fn test_create_agentic_task() {
        let tool = make_tool().await;
        let ctx = test_ctx();
        let args = serde_json::json!({
            "action": "create",
            "title": "Auto-review PR",
            "area_id": "test-area",
            "task_type": "agentic",
            "acceptance_criteria": "All tests pass and coverage > 80%"
        });

        let result = tool.execute(args, &ctx).await.unwrap();
        assert!(result.contains("Task created: Auto-review PR"));
        assert!(result.contains("type: agentic"));
    }

    #[tokio::test]
    async fn test_update_task() {
        let tool = make_tool().await;
        let ctx = test_ctx();

        // Create a task first
        let create_args = serde_json::json!({
            "action": "create",
            "title": "Original title",
            "area_id": "test-area"
        });
        let create_result = tool.execute(create_args, &ctx).await.unwrap();

        // Extract ID from result
        let id = create_result
            .split("ID: ")
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap()
            .trim_end_matches(')');

        // Update
        let update_args = serde_json::json!({
            "action": "update",
            "id": id,
            "title": "Updated title",
            "priority": 1,
            "energy_level": "deep"
        });
        let update_result = tool.execute(update_args, &ctx).await.unwrap();
        assert!(update_result.contains("Updated task: Updated title"));

        // Verify via show
        let show_args = serde_json::json!({ "action": "show", "id": id });
        let show_result = tool.execute(show_args, &ctx).await.unwrap();
        assert!(show_result.contains("Updated title"));
        assert!(show_result.contains("P1"));
        assert!(show_result.contains("Energy: deep"));
    }

    #[tokio::test]
    async fn test_complete_task() {
        let tool = make_tool().await;
        let ctx = test_ctx();

        // Create
        let create_args = serde_json::json!({
            "action": "create",
            "title": "Finish feature",
            "area_id": "test-area"
        });
        let create_result = tool.execute(create_args, &ctx).await.unwrap();
        let id = create_result
            .split("ID: ")
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap()
            .trim_end_matches(')');

        // Complete
        let complete_args = serde_json::json!({ "action": "complete", "id": id });
        let complete_result = tool.execute(complete_args, &ctx).await.unwrap();
        assert!(complete_result.contains("Completed: Finish feature"));

        // Verify status via show
        let show_args = serde_json::json!({ "action": "show", "id": id });
        let show_result = tool.execute(show_args, &ctx).await.unwrap();
        assert!(show_result.contains("Status: done"));
    }

    #[tokio::test]
    async fn test_complete_blocked_task_fails() {
        let tool = make_tool().await;
        let ctx = test_ctx();

        // Create two tasks
        let args1 = serde_json::json!({
            "action": "create", "title": "Blocker", "area_id": "test-area"
        });
        let r1 = tool.execute(args1, &ctx).await.unwrap();
        let id1 = r1
            .split("ID: ")
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap()
            .trim_end_matches(')');

        let args2 = serde_json::json!({
            "action": "create", "title": "Blocked", "area_id": "test-area"
        });
        let r2 = tool.execute(args2, &ctx).await.unwrap();
        let id2 = r2
            .split("ID: ")
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap()
            .trim_end_matches(')');

        // Add dependency: id2 blocked by id1
        let dep_args = serde_json::json!({
            "action": "add_dep",
            "task_id": id2,
            "blocked_by": id1
        });
        tool.execute(dep_args, &ctx).await.unwrap();

        // Try to complete blocked task — should fail
        let complete_args = serde_json::json!({ "action": "complete", "id": id2 });
        let err = tool.execute(complete_args, &ctx).await;
        assert!(err.is_err());
        let err_msg = format!("{}", err.unwrap_err());
        assert!(err_msg.contains("blocked by"));
    }

    #[tokio::test]
    async fn test_delete_task() {
        let tool = make_tool().await;
        let ctx = test_ctx();

        let create_args = serde_json::json!({
            "action": "create", "title": "To delete", "area_id": "test-area"
        });
        let r = tool.execute(create_args, &ctx).await.unwrap();
        let id = r
            .split("ID: ")
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap()
            .trim_end_matches(')');

        let delete_args = serde_json::json!({ "action": "delete", "id": id });
        let result = tool.execute(delete_args, &ctx).await.unwrap();
        assert!(result.contains("Deleted task"));
    }

    #[tokio::test]
    async fn test_search_keyword() {
        let tool = make_tool().await;
        let ctx = test_ctx();

        tool.execute(
            serde_json::json!({ "action": "create", "title": "Fix auth bug", "area_id": "test-area" }),
            &ctx,
        ).await.unwrap();
        tool.execute(
            serde_json::json!({ "action": "create", "title": "Add logging", "area_id": "test-area" }),
            &ctx,
        ).await.unwrap();

        let search_args = serde_json::json!({ "action": "search", "query": "auth" });
        let result = tool.execute(search_args, &ctx).await.unwrap();
        assert!(result.contains("Fix auth bug"));
        assert!(!result.contains("Add logging"));
    }

    #[tokio::test]
    async fn test_summary() {
        let tool = make_tool().await;
        let ctx = test_ctx();

        tool.execute(
            serde_json::json!({ "action": "create", "title": "Task 1", "area_id": "test-area" }),
            &ctx,
        )
        .await
        .unwrap();

        let result = tool
            .execute(serde_json::json!({ "action": "summary" }), &ctx)
            .await
            .unwrap();
        assert!(result.contains("Total tasks: 1"));
    }

    #[tokio::test]
    async fn test_batch_operations() {
        let tool = make_tool().await;
        let ctx = test_ctx();

        let r1 = tool.execute(
            serde_json::json!({ "action": "create", "title": "Batch task 1", "area_id": "test-area" }),
            &ctx,
        ).await.unwrap();
        let id1 = r1
            .split("ID: ")
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap()
            .trim_end_matches(')');

        let r2 = tool.execute(
            serde_json::json!({ "action": "create", "title": "Batch task 2", "area_id": "test-area" }),
            &ctx,
        ).await.unwrap();
        let id2 = r2
            .split("ID: ")
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap()
            .trim_end_matches(')');

        let batch_args = serde_json::json!({
            "action": "batch",
            "operations": [
                { "op": "complete", "id": id1 },
                { "op": "tag", "id": id2, "tags": ["urgent"] }
            ]
        });
        let result = tool.execute(batch_args, &ctx).await.unwrap();
        assert!(result.contains("Completed:"));
        assert!(result.contains("Tagged:"));
    }

    #[tokio::test]
    async fn test_focus_and_unfocus() {
        let tool = make_tool().await;
        let ctx = test_ctx();

        let r = tool.execute(
            serde_json::json!({ "action": "create", "title": "Focus me", "area_id": "test-area" }),
            &ctx,
        ).await.unwrap();
        let id = r
            .split("ID: ")
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap()
            .trim_end_matches(')');

        let focus_result = tool
            .execute(serde_json::json!({ "action": "focus", "id": id }), &ctx)
            .await
            .unwrap();
        assert!(focus_result.contains("Focused on task"));

        let unfocus_result = tool
            .execute(serde_json::json!({ "action": "unfocus", "id": id }), &ctx)
            .await
            .unwrap();
        assert!(unfocus_result.contains("Unfocused task"));
    }

    #[tokio::test]
    async fn test_plan_day() {
        let tool = make_tool().await;
        let ctx = test_ctx();

        tool.execute(
            serde_json::json!({ "action": "create", "title": "Plan task 1", "area_id": "test-area", "priority": 1 }),
            &ctx,
        ).await.unwrap();
        tool.execute(
            serde_json::json!({ "action": "create", "title": "Plan task 2", "area_id": "test-area", "priority": 3 }),
            &ctx,
        ).await.unwrap();

        let result = tool
            .execute(
                serde_json::json!({ "action": "plan_day", "count": 2 }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(result.contains("Daily plan"));
        assert!(result.contains("Plan task 1"));
    }

    #[tokio::test]
    async fn test_unknown_action_returns_error() {
        let tool = make_tool().await;
        let ctx = test_ctx();
        let args = serde_json::json!({ "action": "nonexistent" });
        let err = tool.execute(args, &ctx).await;
        assert!(err.is_err());
    }
}
