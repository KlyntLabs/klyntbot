//! TaskTool — feature-tasks' primary Tool implementation.

mod actions;

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use bus::DomainEventBus;

use crate::config::TasksConfig;
use crate::handlers::EmbeddingHandler;
use crate::types::{Attachment, Task, TimeEntry};
use common::{Result, ToolError};
use storage::TaskRepo;
use tools_core::{
    approval_class::ApprovalClass, ParamExtractor, ProgressHandler, RoutingContext, Tool,
};

/// TaskTool: task CRUD, semantic search, recurrence, focus, and alarms.
pub struct TaskTool {
    pub(crate) repo: TaskRepo,
    pub(crate) area_repo: Option<storage::AreaRepo>,
    pub(crate) max_focus_slots: usize,
    pub(crate) focus_deadline_hours: u64,
    pub(crate) timezone: String,
    pub(crate) embedding_handler: Option<Arc<dyn EmbeddingHandler>>,
    /// LanceDB vector store for semantic search.
    pub(crate) embedding_store: Option<storage::VectorStore>,
    pub(crate) semantic_threshold: f64,
    pub(crate) rrf_k: u32,
    /// Optional progress handler for cascading KR progress on complete.
    pub(crate) progress_handler: Option<Arc<dyn ProgressHandler>>,
    /// Optional domain event bus for cross-feature communication.
    pub(crate) domain_bus: Option<Arc<DomainEventBus>>,
    /// Feature configuration.
    pub(crate) config: TasksConfig,
    /// Optional task_alarms repo — required for `alarms` param on create/update.
    pub(crate) task_alarms_repo: Option<storage::repos::task_alarms::TaskAlarmsRepo>,
    /// Optional FireStore — required for `alarms` param on create/update.
    pub(crate) fire_store: Option<Arc<scheduling::temporal::fire_store::FireStore>>,
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
            embedding_handler: None,
            embedding_store: None,
            semantic_threshold: 0.5,
            rrf_k: 60,
            progress_handler: None,
            domain_bus: None,
            config: TasksConfig::default(),
            task_alarms_repo: None,
            fire_store: None,
        }
    }

    /// Attach the task_alarms repo + scheduler FireStore so the `alarms`
    /// param on create/update materializes user/agent-supplied alarm rules
    /// into the canonical `scheduled_fires` table.
    pub fn with_alarm_writer(
        mut self,
        task_alarms_repo: storage::repos::task_alarms::TaskAlarmsRepo,
        fire_store: Arc<scheduling::temporal::fire_store::FireStore>,
    ) -> Self {
        self.task_alarms_repo = Some(task_alarms_repo);
        self.fire_store = Some(fire_store);
        self
    }

    /// Attach an area repo for auto-resolving area_id on create.
    pub fn with_area_repo(mut self, repo: storage::AreaRepo) -> Self {
        self.area_repo = Some(repo);
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
        self.semantic_threshold = config.search.semantic_threshold;
        self.rrf_k = config.search.rrf_k;
        self.config = config;
        self
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

    /// Parse the `alarms` JSON param into typed `AlarmSpec`s.
    ///
    /// Returns `Ok(None)` if the field is absent (caller should leave alarms unchanged),
    /// `Ok(Some(vec![]))` for explicit empty (clear all alarms),
    /// `Ok(Some(specs))` otherwise.
    pub(crate) fn parse_alarms_param(
        p: &tools_core::ParamExtractor<'_>,
    ) -> Result<Option<Vec<crate::alarms::AlarmSpec>>> {
        let Some(arr) = p.optional_array("alarms")? else {
            return Ok(None);
        };
        let mut specs = Vec::with_capacity(arr.len());
        for v in arr {
            let spec: crate::alarms::AlarmSpec =
                serde_json::from_value(v.clone()).map_err(|e| {
                    ToolError::InvalidParams(format!("invalid alarm spec: {e} (item: {v})"))
                })?;
            specs.push(spec);
        }
        Ok(Some(specs))
    }

    /// Materialize alarm specs for a task. No-op if `task_alarms_repo` /
    /// `fire_store` weren't injected. Cancels existing alarms first when
    /// `replace` is true (use for updates).
    pub(crate) async fn apply_alarm_specs(
        &self,
        task_id: &str,
        due_date: Option<jiff::Timestamp>,
        specs: &[crate::alarms::AlarmSpec],
        replace: bool,
    ) -> Result<usize> {
        let (Some(repo), Some(store)) = (&self.task_alarms_repo, &self.fire_store) else {
            tracing::debug!("alarms: task {task_id} provided alarms but no writer wired; skipping");
            return Ok(0);
        };
        if replace {
            crate::alarms::cancel_for_task(task_id, repo, store)
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("alarm cancel failed: {e}")))?;
        }
        crate::alarms::materialize_for_task(task_id, due_date, specs, &self.timezone, repo, store)
            .await
            .map_err(|e| {
                ToolError::ExecutionFailed(format!("alarm materialize failed: {e}")).into()
            })
    }
}

#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &str {
        "tasks"
    }

    fn allowed_channels(&self) -> common::ChannelMask {
        common::ChannelMask::NON_CODING
    }

    fn description(&self) -> &str {
        "Manage individual to-do items and action items. Create, complete, update, delete, list, and search tasks. Supports priorities, dependencies, time logging, and recurring tasks. NOT for project containers or OKR objectives."
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
                        "recur", "list_recurring", "delete_recurring"
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
                    "enum": ["manual"],
                    "description": "Task type (default: manual)"
                },
                "energy_level": {
                    "type": "string",
                    "enum": ["low", "medium", "high", "deep"],
                    "description": "Energy level required"
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
                "alarms": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["relative_before", "civil_time", "absolute"] },
                            "offset_secs": { "type": "integer", "description": "for relative_before: seconds before due_date" },
                            "day_offset": { "type": "integer", "description": "for civil_time: day offset (-1 = day before)" },
                            "time_of_day": { "type": "string", "description": "for civil_time: 'HH:MM'" },
                            "timezone": { "type": "string", "description": "for civil_time: IANA tz, defaults to config.timezone" },
                            "fire_at": { "type": "string", "description": "for absolute: ISO 8601 timestamp" },
                            "channels": { "type": "array", "items": { "type": "string" }, "description": "channel names: os_native|tray|telegram|discord|email" },
                            "priority": { "type": "string", "enum": ["normal", "urgent"] }
                        },
                        "required": ["type"]
                    },
                    "description": "Alarm rules for this task (replaces existing alarms on update). Spec §8.3."
                },
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
            _ => Err(ToolError::InvalidParams(format!("Unknown action: {action}")).into()),
        }
    }

    fn approval_class(&self, args: &Value) -> ApprovalClass {
        match args.get("action").and_then(|v| v.as_str()) {
            Some(
                "create" | "update" | "complete" | "reopen" | "focus" | "unfocus" | "log_time"
                | "add_dep" | "remove_dep" | "batch" | "recur" | "list_recurring",
            ) => ApprovalClass::Sensitive,
            Some("delete" | "delete_recurring") => ApprovalClass::Destructive,
            _ => ApprovalClass::Safe,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            r#"CREATE TABLE IF NOT EXISTS task_estimation_history (
                id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                estimated_minutes INTEGER NOT NULL, actual_minutes INTEGER NOT NULL,
                deviation_pct REAL NOT NULL DEFAULT 0.0, complexity_score INTEGER,
                energy_level TEXT, tags TEXT NOT NULL DEFAULT '[]',
                project_id TEXT,
                completed_at INTEGER NOT NULL
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
    async fn test_deferral_publishes_task_deferred() {
        use bus::{DomainEvent, DomainEventBus};
        use std::sync::Arc;

        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let db = pool.inner().clone();
        create_test_tables(&db).await;
        let repo = storage::TaskRepo::new(db);
        let bus = Arc::new(DomainEventBus::new(32));
        let mut rx = bus.subscribe();
        let tool = TaskTool::new(repo, 3, 8, "UTC".to_string()).with_domain_bus(Arc::clone(&bus));
        let ctx = test_ctx();

        let create = tool
            .execute(
                serde_json::json!({
                    "action": "create",
                    "title": "defer probe",
                    "area_id": "test-area",
                    "due_date": "2026-05-01T12:00:00Z"
                }),
                &ctx,
            )
            .await
            .unwrap();
        let id = create
            .split("ID: ")
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap()
            .trim_end_matches(')');

        // Drain any events from creation
        while rx.try_recv().is_ok() {}

        tool.execute(
            serde_json::json!({
                "action": "update",
                "id": id,
                "due_date": "2026-05-10T12:00:00Z"
            }),
            &ctx,
        )
        .await
        .unwrap();

        let mut saw_deferred = false;
        while let Ok(ev) = rx.try_recv() {
            if matches!(ev, DomainEvent::TaskDeferred { .. }) {
                saw_deferred = true;
                break;
            }
        }
        assert!(
            saw_deferred,
            "expected DomainEvent::TaskDeferred after due-date moved later"
        );
    }

    #[tokio::test]
    async fn test_pull_forward_does_not_publish_task_deferred() {
        use bus::{DomainEvent, DomainEventBus};
        use std::sync::Arc;

        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let db = pool.inner().clone();
        create_test_tables(&db).await;
        let repo = storage::TaskRepo::new(db);
        let bus = Arc::new(DomainEventBus::new(32));
        let mut rx = bus.subscribe();
        let tool = TaskTool::new(repo, 3, 8, "UTC".to_string()).with_domain_bus(Arc::clone(&bus));
        let ctx = test_ctx();

        let create = tool
            .execute(
                serde_json::json!({
                    "action": "create",
                    "title": "pull forward",
                    "area_id": "test-area",
                    "due_date": "2026-05-10T12:00:00Z"
                }),
                &ctx,
            )
            .await
            .unwrap();
        let id = create
            .split("ID: ")
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap()
            .trim_end_matches(')');
        while rx.try_recv().is_ok() {}

        tool.execute(
            serde_json::json!({
                "action": "update",
                "id": id,
                "due_date": "2026-05-01T12:00:00Z"
            }),
            &ctx,
        )
        .await
        .unwrap();

        while let Ok(ev) = rx.try_recv() {
            assert!(
                !matches!(ev, DomainEvent::TaskDeferred { .. }),
                "pulling a task earlier should NOT emit TaskDeferred"
            );
        }
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
    async fn test_unknown_action_returns_error() {
        let tool = make_tool().await;
        let ctx = test_ctx();
        let args = serde_json::json!({ "action": "nonexistent" });
        let err = tool.execute(args, &ctx).await;
        assert!(err.is_err());
    }
}
