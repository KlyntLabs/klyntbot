//! Shared test utilities for dashboard infrastructure and GraphQL tests.

use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::{broadcast, RwLock};

use dashboard::events::{store_events::ObservableTodoStore, DashboardEvent, DashboardEventBus};
use dashboard::graphql::{build_schema, DashboardContext, DashboardSchema};
use goal::GoalStore;
use plan::PlanStore;
use tools::project_store::ProjectStore;
use tools::project_types::{Project, ProjectColor, ProjectStatus};
use tools::todo_store::TodoStore;
use tools::todo_types::{Todo, TodoStatus};

/// Create a temporary DashboardEventBus (capacity 64).
pub fn create_test_bus() -> Arc<DashboardEventBus> {
    Arc::new(DashboardEventBus::new(64))
}

/// Create a temporary TodoStore backed by a temp directory.
/// Returns the store and the TempDir (keep it alive!).
pub fn create_test_todo_store(tmp: &TempDir) -> Arc<RwLock<TodoStore>> {
    let path = tmp.path().join("todos.jsonl");
    Arc::new(RwLock::new(TodoStore::new(path)))
}

/// Create an ObservableTodoStore wrapping a temp TodoStore.
pub fn create_observable_store(tmp: &TempDir, bus: Arc<DashboardEventBus>) -> ObservableTodoStore {
    let inner = create_test_todo_store(tmp);
    ObservableTodoStore::new(inner, bus)
}

// ── GraphQL test helpers ──────────────────────────────────────────────────────

/// Create a full `DashboardContext` backed by temp-dir stores.
///
/// Keep the returned `TempDir` alive for the duration of the test.
pub async fn create_test_context(tmp: &TempDir) -> DashboardContext {
    let data_dir = tmp.path();
    let todo_store = Arc::new(RwLock::new(TodoStore::new(data_dir.join("todos.jsonl"))));
    let project_store = Arc::new(RwLock::new(ProjectStore::new(
        data_dir.join("projects.jsonl"),
    )));
    let goal_store = Arc::new(RwLock::new(GoalStore::new(data_dir.join("goals.jsonl"))));
    let plan_store = Arc::new(RwLock::new(PlanStore::new(data_dir.join("plans.jsonl"))));
    let (event_tx, _) = broadcast::channel::<DashboardEvent>(64);
    let config = Arc::new(RwLock::new(config::Config::default()));
    DashboardContext {
        todo_store,
        project_store,
        goal_store,
        plan_store,
        event_tx,
        config,
    }
}

/// Build a complete, executable GraphQL schema from a `DashboardContext`.
pub fn create_test_schema(ctx: DashboardContext) -> DashboardSchema {
    build_schema().data(ctx).finish()
}

// ── Project factory ───────────────────────────────────────────────────────────

/// Build a minimal Project for testing.
pub fn make_project(name: &str) -> Project {
    use chrono::Utc;
    let now = Utc::now();
    Project {
        id: Project::generate_id(),
        name: name.to_string(),
        description: None,
        color: ProjectColor::Blue,
        tags: vec![],
        status: ProjectStatus::Active,
        created_at: now,
        updated_at: now,
    }
}

/// Build a minimal Todo for testing.
pub fn make_todo(title: &str) -> Todo {
    use chrono::Utc;
    let now = Utc::now();
    Todo {
        id: uuid::Uuid::new_v4().to_string(),
        title: title.to_string(),
        description: None,
        priority: None,
        due_date: None,
        tags: vec![],
        status: TodoStatus::Todo,
        focused_at: None,
        focus_deadline: None,
        focus_expired_count: 0,
        created_at: now,
        updated_at: now,
        completed_at: None,
        parent_id: None,
        project_id: None,
        attachments: vec![],
        time_entries: vec![],
        total_tracked_secs: 0,
        estimated_minutes: None,
        calendar_event_uid: None,
        last_reminded_at: None,
        recurrence_rule: None,
        recurrence_parent_id: None,
        is_template: false,
        next_instance_date: None,
        blocked_by: vec![],
        blocks: vec![],
    }
}
