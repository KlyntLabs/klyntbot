//! Sprint 2 integration tests — Task Dependencies & Recurring Tasks
//!
//! Tests for the dependency management and recurring task systems
//! introduced in Sprint 2. Covers:
//!
//! ## Dependencies
//! - Adding/removing dependencies (bidirectional link maintenance)
//! - Cycle detection (DFS-based)
//! - Completion blocking (incomplete blockers prevent completion)
//! - Unblock notifications on completion
//! - Tree view dependency display
//! - Edge cases (self-dep, missing task, duplicate dep, etc.)
//!
//! ## Recurring Tasks
//! - RRULE validation and parsing
//! - Template store methods (list_templates, filtering, next_instance_date)
//! - Template filtering from normal list()
//! - RRULE humanization

use chrono::{TimeZone, Utc};
use serde_json::json;
use std::sync::Arc;
use storage::{StoragePool, TodoRepo};
use tempfile::TempDir;
use tokio::sync::RwLock;
use tools::{
    rrule_utils,
    todo::TodoTool,
    todo_store::TodoStore,
    todo_types::{Todo, TodoFilter, TodoPatch, TodoStatus},
    RoutingContext, Tool,
};

// ─── Test helpers ──────────────────────────────────────────────

fn create_test_todo(title: &str) -> Todo {
    Todo {
        id: Todo::generate_id(),
        title: title.to_string(),
        description: None,
        priority: None,
        due_date: None,
        tags: vec![],
        status: TodoStatus::Todo,
        focused_at: None,
        focus_deadline: None,
        focus_expired_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        completed_at: None,
        parent_id: None,
        project_id: None,
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
    }
}

async fn create_store() -> (TodoStore, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("todos.jsonl");
    let store = TodoStore::new(file_path);
    (store, temp_dir)
}

async fn create_tool_and_store() -> (TodoTool, Arc<RwLock<TodoStore>>, TempDir, RoutingContext) {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("todos.jsonl");
    let store = Arc::new(RwLock::new(TodoStore::new(file_path)));
    let pool = StoragePool::connect_lazy("postgres://localhost/klyntbot_test").unwrap();
    let repo = TodoRepo::new(pool.inner().clone());
    let tool = TodoTool::new(repo, 3, 18, "UTC".to_string());
    let ctx = RoutingContext::new(
        common::ChannelName::new("test"),
        common::ChatId::new("test-chat"),
    );
    (tool, store, temp_dir, ctx)
}

// ─── Store-level: add_dependency ───────────────────────────────

#[tokio::test]
async fn test_add_dependency_creates_bidirectional_links() {
    let (mut store, _dir) = create_store().await;
    let task_a = store.add(create_test_todo("Task A")).await.unwrap();
    let task_b = store.add(create_test_todo("Task B")).await.unwrap();

    // A is blocked by B
    store.add_dependency(&task_a.id, &task_b.id).await.unwrap();

    let a = store.get(&task_a.id).await.unwrap().unwrap();
    let b = store.get(&task_b.id).await.unwrap().unwrap();

    assert!(
        a.blocked_by.contains(&task_b.id),
        "A should list B in blocked_by"
    );
    assert!(b.blocks.contains(&task_a.id), "B should list A in blocks");
}

#[tokio::test]
async fn test_add_dependency_idempotent() {
    let (mut store, _dir) = create_store().await;
    let task_a = store.add(create_test_todo("Task A")).await.unwrap();
    let task_b = store.add(create_test_todo("Task B")).await.unwrap();

    store.add_dependency(&task_a.id, &task_b.id).await.unwrap();
    store.add_dependency(&task_a.id, &task_b.id).await.unwrap(); // duplicate

    let a = store.get(&task_a.id).await.unwrap().unwrap();
    assert_eq!(
        a.blocked_by.iter().filter(|id| *id == &task_b.id).count(),
        1,
        "Should not duplicate dependency"
    );
}

#[tokio::test]
async fn test_add_dependency_self_reference_fails() {
    let (mut store, _dir) = create_store().await;
    let task = store.add(create_test_todo("Self")).await.unwrap();

    let result = store.add_dependency(&task.id, &task.id).await;
    assert!(result.is_err(), "Self-dependency should fail");
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("Cannot depend on self"), "Error: {}", err);
}

#[tokio::test]
async fn test_add_dependency_missing_task_fails() {
    let (mut store, _dir) = create_store().await;
    let task = store.add(create_test_todo("Real")).await.unwrap();

    let result = store.add_dependency(&task.id, "nonexistent").await;
    assert!(result.is_err(), "Missing blocker should fail");

    let result2 = store.add_dependency("nonexistent", &task.id).await;
    assert!(result2.is_err(), "Missing task should fail");
}

// ─── Store-level: cycle detection ──────────────────────────────

#[tokio::test]
async fn test_would_create_cycle_simple() {
    let (mut store, _dir) = create_store().await;
    let a = store.add(create_test_todo("A")).await.unwrap();
    let b = store.add(create_test_todo("B")).await.unwrap();

    // A blocked by B
    store.add_dependency(&a.id, &b.id).await.unwrap();

    // Adding B blocked by A would create cycle: A→B→A
    let has_cycle = store.would_create_cycle(&b.id, &a.id).await.unwrap();
    assert!(has_cycle, "B→A should create a cycle since A→B exists");
}

#[tokio::test]
async fn test_would_create_cycle_transitive() {
    let (mut store, _dir) = create_store().await;
    let a = store.add(create_test_todo("A")).await.unwrap();
    let b = store.add(create_test_todo("B")).await.unwrap();
    let c = store.add(create_test_todo("C")).await.unwrap();

    // Chain: A blocked by B, B blocked by C
    store.add_dependency(&a.id, &b.id).await.unwrap();
    store.add_dependency(&b.id, &c.id).await.unwrap();

    // C blocked by A would create: A→B→C→A
    let has_cycle = store.would_create_cycle(&c.id, &a.id).await.unwrap();
    assert!(has_cycle, "Transitive cycle should be detected");
}

#[tokio::test]
async fn test_would_create_cycle_no_cycle() {
    let (mut store, _dir) = create_store().await;
    let a = store.add(create_test_todo("A")).await.unwrap();
    let b = store.add(create_test_todo("B")).await.unwrap();
    let c = store.add(create_test_todo("C")).await.unwrap();

    // A blocked by B
    store.add_dependency(&a.id, &b.id).await.unwrap();

    // C blocked by B is fine (no cycle)
    let has_cycle = store.would_create_cycle(&c.id, &b.id).await.unwrap();
    assert!(!has_cycle, "Parallel dep should not create cycle");
}

#[tokio::test]
async fn test_add_dependency_rejects_cycle() {
    let (mut store, _dir) = create_store().await;
    let a = store.add(create_test_todo("A")).await.unwrap();
    let b = store.add(create_test_todo("B")).await.unwrap();

    store.add_dependency(&a.id, &b.id).await.unwrap();

    let result = store.add_dependency(&b.id, &a.id).await;
    assert!(
        result.is_err(),
        "Cycle-creating dependency should be rejected"
    );
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("cycle"), "Error should mention cycle: {}", err);
}

// ─── Store-level: remove_dependency ────────────────────────────

#[tokio::test]
async fn test_remove_dependency_cleans_both_sides() {
    let (mut store, _dir) = create_store().await;
    let a = store.add(create_test_todo("A")).await.unwrap();
    let b = store.add(create_test_todo("B")).await.unwrap();

    store.add_dependency(&a.id, &b.id).await.unwrap();
    store.remove_dependency(&a.id, &b.id).await.unwrap();

    let a = store.get(&a.id).await.unwrap().unwrap();
    let b = store.get(&b.id).await.unwrap().unwrap();

    assert!(
        a.blocked_by.is_empty(),
        "A.blocked_by should be empty after removal"
    );
    assert!(
        b.blocks.is_empty(),
        "B.blocks should be empty after removal"
    );
}

#[tokio::test]
async fn test_remove_nonexistent_dependency_is_safe() {
    let (mut store, _dir) = create_store().await;
    let a = store.add(create_test_todo("A")).await.unwrap();
    let b = store.add(create_test_todo("B")).await.unwrap();

    // Remove a dependency that never existed — should not error
    let result = store.remove_dependency(&a.id, &b.id).await;
    assert!(result.is_ok(), "Removing nonexistent dep should be a no-op");
}

// ─── Store-level: incomplete_blockers ──────────────────────────

#[tokio::test]
async fn test_incomplete_blockers_returns_only_active() {
    let (mut store, _dir) = create_store().await;
    let task = store.add(create_test_todo("Main")).await.unwrap();
    let blocker1 = store.add(create_test_todo("Blocker 1")).await.unwrap();
    let blocker2 = store.add(create_test_todo("Blocker 2")).await.unwrap();

    store.add_dependency(&task.id, &blocker1.id).await.unwrap();
    store.add_dependency(&task.id, &blocker2.id).await.unwrap();

    // Complete blocker1
    store
        .update(
            &blocker1.id,
            TodoPatch {
                status: Some(TodoStatus::Done),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let incomplete = store.incomplete_blockers(&task.id).await.unwrap();
    assert_eq!(incomplete.len(), 1);
    assert_eq!(incomplete[0].id, blocker2.id);
}

#[tokio::test]
async fn test_incomplete_blockers_empty_when_all_done() {
    let (mut store, _dir) = create_store().await;
    let task = store.add(create_test_todo("Main")).await.unwrap();
    let blocker = store.add(create_test_todo("Blocker")).await.unwrap();

    store.add_dependency(&task.id, &blocker.id).await.unwrap();

    store
        .update(
            &blocker.id,
            TodoPatch {
                status: Some(TodoStatus::Done),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let incomplete = store.incomplete_blockers(&task.id).await.unwrap();
    assert!(incomplete.is_empty());
}

#[tokio::test]
async fn test_incomplete_blockers_nonexistent_task() {
    let (mut store, _dir) = create_store().await;
    let incomplete = store.incomplete_blockers("nonexistent").await.unwrap();
    assert!(
        incomplete.is_empty(),
        "Nonexistent task should return empty blockers"
    );
}

// ─── Store-level: persistence ──────────────────────────────────

#[tokio::test]
async fn test_dependencies_persist_across_reloads() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("todos.jsonl");

    let (a_id, b_id) = {
        let mut store = TodoStore::new(file_path.clone());
        let a = store.add(create_test_todo("A")).await.unwrap();
        let b = store.add(create_test_todo("B")).await.unwrap();
        store.add_dependency(&a.id, &b.id).await.unwrap();
        (a.id, b.id)
    };

    // Reload from disk
    let mut store2 = TodoStore::new(file_path);
    let a = store2.get(&a_id).await.unwrap().unwrap();
    let b = store2.get(&b_id).await.unwrap().unwrap();

    assert!(a.blocked_by.contains(&b_id));
    assert!(b.blocks.contains(&a_id));
}

// ─── Store-level: delete cascades ──────────────────────────────

#[tokio::test]
async fn test_delete_task_removes_from_dependency_links() {
    let (mut store, _dir) = create_store().await;
    let a = store.add(create_test_todo("A")).await.unwrap();
    let b = store.add(create_test_todo("B")).await.unwrap();
    let c = store.add(create_test_todo("C")).await.unwrap();

    // B blocks both A and C
    store.add_dependency(&a.id, &b.id).await.unwrap();
    store.add_dependency(&c.id, &b.id).await.unwrap();

    // Delete B — should clean up A.blocked_by and C.blocked_by
    store.delete(&b.id).await.unwrap();

    let a = store.get(&a.id).await.unwrap().unwrap();
    let c = store.get(&c.id).await.unwrap().unwrap();
    assert!(
        a.blocked_by.is_empty(),
        "Deleting B should clean A.blocked_by"
    );
    assert!(
        c.blocked_by.is_empty(),
        "Deleting B should clean C.blocked_by"
    );
}

// ─── Store-level: complex dependency graphs ────────────────────

#[tokio::test]
async fn test_diamond_dependency_graph() {
    let (mut store, _dir) = create_store().await;
    //     A
    //    / \
    //   B   C
    //    \ /
    //     D
    let a = store.add(create_test_todo("A")).await.unwrap();
    let b = store.add(create_test_todo("B")).await.unwrap();
    let c = store.add(create_test_todo("C")).await.unwrap();
    let d = store.add(create_test_todo("D")).await.unwrap();

    // A blocked by B and C; B and C blocked by D
    store.add_dependency(&a.id, &b.id).await.unwrap();
    store.add_dependency(&a.id, &c.id).await.unwrap();
    store.add_dependency(&b.id, &d.id).await.unwrap();
    store.add_dependency(&c.id, &d.id).await.unwrap();

    // A should have 2 blockers
    let a_blockers = store.incomplete_blockers(&a.id).await.unwrap();
    assert_eq!(a_blockers.len(), 2);

    // D blocks 2 tasks
    let d = store.get(&d.id).await.unwrap().unwrap();
    assert_eq!(d.blocks.len(), 2);

    // D blocked by A would create cycle
    let has_cycle = store.would_create_cycle(&d.id, &a.id).await.unwrap();
    assert!(has_cycle, "D→A would close the diamond into a cycle");
}

#[tokio::test]
async fn test_chain_dependency_no_false_cycle() {
    let (mut store, _dir) = create_store().await;
    // Linear chain: A → B → C → D (A blocked by B, B by C, C by D)
    let a = store.add(create_test_todo("A")).await.unwrap();
    let b = store.add(create_test_todo("B")).await.unwrap();
    let c = store.add(create_test_todo("C")).await.unwrap();
    let d = store.add(create_test_todo("D")).await.unwrap();

    store.add_dependency(&a.id, &b.id).await.unwrap();
    store.add_dependency(&b.id, &c.id).await.unwrap();
    store.add_dependency(&c.id, &d.id).await.unwrap();

    // Adding E blocked by D is fine
    let e = store.add(create_test_todo("E")).await.unwrap();
    let has_cycle = store.would_create_cycle(&e.id, &d.id).await.unwrap();
    assert!(!has_cycle);

    store.add_dependency(&e.id, &d.id).await.unwrap();
    let e = store.get(&e.id).await.unwrap().unwrap();
    assert!(e.blocked_by.contains(&d.id));
}

// ─── TodoTool-level: add_dependency action ─────────────────────

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_tool_add_dependency_action() {
    let (tool, store, _dir, ctx) = create_tool_and_store().await;

    let (a_id, b_id) = {
        let mut s = store.write().await;
        let a = s.add(create_test_todo("Build frontend")).await.unwrap();
        let b = s.add(create_test_todo("Design API")).await.unwrap();
        (a.id, b.id)
    };

    let params = json!({
        "action": "add_dependency",
        "task_id": a_id,
        "blocked_by": b_id
    });

    let result = tool.execute(params, &ctx).await.unwrap();

    assert!(result.contains("blocked by"), "Result: {}", result);
    assert!(result.contains("Build frontend"));
    assert!(result.contains("Design API"));
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_tool_remove_dependency_action() {
    let (tool, store, _dir, ctx) = create_tool_and_store().await;

    let (a_id, b_id) = {
        let mut s = store.write().await;
        let a = s.add(create_test_todo("Build frontend")).await.unwrap();
        let b = s.add(create_test_todo("Design API")).await.unwrap();
        s.add_dependency(&a.id, &b.id).await.unwrap();
        (a.id, b.id)
    };

    let params = json!({
        "action": "remove_dependency",
        "task_id": a_id,
        "blocked_by": b_id
    });

    let result = tool.execute(params, &ctx).await.unwrap();
    assert!(
        result.contains("no longer blocked by"),
        "Result: {}",
        result
    );
}

// ─── TodoTool-level: complete with blockers ────────────────────

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_tool_complete_blocked_task_fails() {
    let (tool, store, _dir, ctx) = create_tool_and_store().await;

    let task_id = {
        let mut s = store.write().await;
        let task = s.add(create_test_todo("Blocked task")).await.unwrap();
        let blocker = s.add(create_test_todo("Blocker")).await.unwrap();
        s.add_dependency(&task.id, &blocker.id).await.unwrap();
        task.id
    };

    let params = json!({
        "action": "complete",
        "id": task_id
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err(), "Completing a blocked task should fail");
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("blocked by"),
        "Error should mention blockers: {}",
        err
    );
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_tool_complete_unblocked_task_succeeds() {
    let (tool, store, _dir, ctx) = create_tool_and_store().await;

    let task_id = {
        let mut s = store.write().await;
        let task = s.add(create_test_todo("Task")).await.unwrap();
        let blocker = s.add(create_test_todo("Blocker")).await.unwrap();
        s.add_dependency(&task.id, &blocker.id).await.unwrap();

        // Complete the blocker first
        s.update(
            &blocker.id,
            TodoPatch {
                status: Some(TodoStatus::Done),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        task.id
    };

    let params = json!({
        "action": "complete",
        "id": task_id
    });

    let result = tool.execute(params, &ctx).await.unwrap();
    assert!(result.contains("Completed"), "Result: {}", result);
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_tool_complete_reports_newly_unblocked() {
    let (tool, store, _dir, ctx) = create_tool_and_store().await;

    let blocker_id = {
        let mut s = store.write().await;
        let blocker = s.add(create_test_todo("Blocker")).await.unwrap();
        let waiting = s.add(create_test_todo("Waiting task")).await.unwrap();
        s.add_dependency(&waiting.id, &blocker.id).await.unwrap();
        blocker.id
    };

    let params = json!({
        "action": "complete",
        "id": blocker_id
    });

    let result = tool.execute(params, &ctx).await.unwrap();

    assert!(result.contains("Completed"), "Result: {}", result);
    assert!(
        result.contains("Unblocked") || result.contains("unblocked"),
        "Should report unblocked tasks: {}",
        result
    );
    assert!(
        result.contains("Waiting task"),
        "Should name the unblocked task: {}",
        result
    );
}

// ─── TodoTool-level: tree shows dependencies ───────────────────

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_tool_tree_shows_dependency_info() {
    let (tool, store, _dir, ctx) = create_tool_and_store().await;

    {
        let mut s = store.write().await;
        let task = s.add(create_test_todo("Build")).await.unwrap();
        let dep = s.add(create_test_todo("Design")).await.unwrap();
        s.add_dependency(&task.id, &dep.id).await.unwrap();
    }

    let params = json!({
        "action": "tree"
    });

    let result = tool.execute(params, &ctx).await.unwrap();

    // Tree output should show both tasks
    assert!(
        result.contains("Build"),
        "Tree should show task: {}",
        result
    );
    assert!(
        result.contains("Design"),
        "Tree should show dep: {}",
        result
    );
}

// ─── Multiple dependencies ─────────────────────────────────────

#[tokio::test]
async fn test_multiple_blockers_all_must_complete() {
    let (mut store, _dir) = create_store().await;
    let main = store.add(create_test_todo("Main")).await.unwrap();
    let b1 = store.add(create_test_todo("B1")).await.unwrap();
    let b2 = store.add(create_test_todo("B2")).await.unwrap();
    let b3 = store.add(create_test_todo("B3")).await.unwrap();

    store.add_dependency(&main.id, &b1.id).await.unwrap();
    store.add_dependency(&main.id, &b2.id).await.unwrap();
    store.add_dependency(&main.id, &b3.id).await.unwrap();

    // Complete b1 and b3, leave b2
    store
        .update(
            &b1.id,
            TodoPatch {
                status: Some(TodoStatus::Done),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    store
        .update(
            &b3.id,
            TodoPatch {
                status: Some(TodoStatus::Done),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let incomplete = store.incomplete_blockers(&main.id).await.unwrap();
    assert_eq!(incomplete.len(), 1);
    assert_eq!(incomplete[0].id, b2.id);
}

#[tokio::test]
async fn test_archived_blocker_not_counted_as_incomplete() {
    let (mut store, _dir) = create_store().await;
    let main = store.add(create_test_todo("Main")).await.unwrap();
    let blocker = store.add(create_test_todo("Blocker")).await.unwrap();

    store.add_dependency(&main.id, &blocker.id).await.unwrap();

    // Archive the blocker
    store
        .update(
            &blocker.id,
            TodoPatch {
                status: Some(TodoStatus::Archived),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let incomplete = store.incomplete_blockers(&main.id).await.unwrap();
    assert!(
        incomplete.is_empty(),
        "Archived blocker should not count as incomplete"
    );
}

// ═══════════════════════════════════════════════════════════════
// RECURRING TASKS — RRULE validation, templates, store methods
// ═══════════════════════════════════════════════════════════════

// ─── RRULE validation ──────────────────────────────────────────

#[test]
fn test_rrule_validate_daily() {
    assert!(rrule_utils::validate_rrule("FREQ=DAILY").is_ok());
}

#[test]
fn test_rrule_validate_weekly_with_byday() {
    assert!(rrule_utils::validate_rrule("FREQ=WEEKLY;BYDAY=MO,WE,FR").is_ok());
}

#[test]
fn test_rrule_validate_monthly_with_interval() {
    assert!(rrule_utils::validate_rrule("FREQ=MONTHLY;INTERVAL=2;BYMONTHDAY=15").is_ok());
}

#[test]
fn test_rrule_validate_accepts_count() {
    assert!(rrule_utils::validate_rrule("FREQ=DAILY;COUNT=10").is_ok());
}

#[test]
fn test_rrule_validate_accepts_until() {
    assert!(rrule_utils::validate_rrule("FREQ=DAILY;UNTIL=20250101T000000Z").is_ok());
}

#[test]
fn test_rrule_validate_accepts_exdate() {
    assert!(rrule_utils::validate_rrule("FREQ=DAILY;EXDATE=20250101T000000Z").is_ok());
}

#[test]
fn test_rrule_validate_rejects_bysetpos() {
    assert!(rrule_utils::validate_rrule("FREQ=MONTHLY;BYDAY=MO;BYSETPOS=1").is_err());
}

#[test]
fn test_rrule_validate_invalid_syntax() {
    assert!(rrule_utils::validate_rrule("NOT_A_VALID_RULE").is_err());
}

// ─── RRULE next_occurrence ─────────────────────────────────────

#[test]
fn test_rrule_next_occurrence_daily() {
    let after = Utc.with_ymd_and_hms(2025, 6, 1, 9, 0, 0).unwrap();
    let next = rrule_utils::next_occurrence("FREQ=DAILY", after)
        .unwrap()
        .unwrap();
    assert!(next > after, "Next occurrence should be after start");
    let diff = next - after;
    assert_eq!(diff.num_days(), 1, "Daily recurrence should be 1 day later");
}

#[test]
fn test_rrule_next_occurrence_weekly() {
    let after = Utc.with_ymd_and_hms(2025, 6, 1, 9, 0, 0).unwrap();
    let next = rrule_utils::next_occurrence("FREQ=WEEKLY;INTERVAL=2", after)
        .unwrap()
        .unwrap();
    assert!(next > after);
    let diff = next - after;
    assert!(
        diff.num_days() >= 14,
        "Bi-weekly should be 14+ days later, got {}",
        diff.num_days()
    );
}

// ─── RRULE should_spawn_instance ───────────────────────────────

#[test]
fn test_should_spawn_when_past_due() {
    let now = Utc::now();
    let past = now - chrono::Duration::hours(1);
    assert!(rrule_utils::should_spawn_instance(Some(past), now));
}

#[test]
fn test_should_not_spawn_when_future() {
    let now = Utc::now();
    let future = now + chrono::Duration::hours(1);
    assert!(!rrule_utils::should_spawn_instance(Some(future), now));
}

#[test]
fn test_should_not_spawn_when_none() {
    assert!(!rrule_utils::should_spawn_instance(None, Utc::now()));
}

#[test]
fn test_should_spawn_when_exactly_now() {
    let now = Utc::now();
    assert!(rrule_utils::should_spawn_instance(Some(now), now));
}

// ─── RRULE humanization ────────────────────────────────────────

#[test]
fn test_humanize_daily() {
    assert_eq!(rrule_utils::humanize_rrule("FREQ=DAILY"), "Every day");
}

#[test]
fn test_humanize_daily_with_interval() {
    assert_eq!(
        rrule_utils::humanize_rrule("FREQ=DAILY;INTERVAL=3"),
        "Every 3 days"
    );
}

#[test]
fn test_humanize_weekly_with_byday() {
    let result = rrule_utils::humanize_rrule("FREQ=WEEKLY;BYDAY=MO,WE,FR");
    assert!(result.contains("Every week"));
    assert!(result.contains("Monday"));
    assert!(result.contains("Wednesday"));
    assert!(result.contains("Friday"));
}

#[test]
fn test_humanize_monthly_with_bymonthday() {
    let result = rrule_utils::humanize_rrule("FREQ=MONTHLY;BYMONTHDAY=1");
    assert!(result.contains("Every month"));
    assert!(result.contains("day 1"));
}

#[test]
fn test_humanize_yearly() {
    assert_eq!(rrule_utils::humanize_rrule("FREQ=YEARLY"), "Every year");
}

// ─── Template store methods ────────────────────────────────────

#[tokio::test]
async fn test_list_templates_returns_only_templates() {
    let (mut store, _dir) = create_store().await;

    // Add a normal task
    store.add(create_test_todo("Normal task")).await.unwrap();

    // Add a template task
    let mut template = create_test_todo("Daily standup");
    template.is_template = true;
    template.recurrence_rule = Some("FREQ=DAILY".to_string());
    store.add(template).await.unwrap();

    let templates = store.list_templates().await.unwrap();
    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].title, "Daily standup");
    assert!(templates[0].is_template);
}

#[tokio::test]
async fn test_list_templates_empty_when_none() {
    let (mut store, _dir) = create_store().await;
    store.add(create_test_todo("Normal")).await.unwrap();

    let templates = store.list_templates().await.unwrap();
    assert!(templates.is_empty());
}

#[tokio::test]
async fn test_templates_filtered_from_normal_list() {
    let (mut store, _dir) = create_store().await;

    store.add(create_test_todo("Normal task")).await.unwrap();

    let mut template = create_test_todo("Weekly review");
    template.is_template = true;
    template.recurrence_rule = Some("FREQ=WEEKLY".to_string());
    store.add(template).await.unwrap();

    // Default filter should exclude templates
    let normal = store.list(&TodoFilter::default()).await.unwrap();
    assert_eq!(normal.len(), 1);
    assert_eq!(normal[0].title, "Normal task");

    // include_templates should show both
    let all = store
        .list(&TodoFilter {
            include_templates: true,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(all.len(), 2);
}

#[tokio::test]
async fn test_update_next_instance_date() {
    let (mut store, _dir) = create_store().await;

    let mut template = create_test_todo("Daily standup");
    template.is_template = true;
    template.recurrence_rule = Some("FREQ=DAILY".to_string());
    let saved = store.add(template).await.unwrap();

    assert!(saved.next_instance_date.is_none());

    let next = Utc.with_ymd_and_hms(2025, 7, 1, 9, 0, 0).unwrap();
    store
        .update_next_instance_date(&saved.id, Some(next))
        .await
        .unwrap();

    let updated = store.get(&saved.id).await.unwrap().unwrap();
    assert_eq!(updated.next_instance_date, Some(next));
}

#[tokio::test]
async fn test_update_next_instance_date_to_none() {
    let (mut store, _dir) = create_store().await;

    let mut template = create_test_todo("Monthly review");
    template.is_template = true;
    template.next_instance_date = Some(Utc::now());
    let saved = store.add(template).await.unwrap();

    store
        .update_next_instance_date(&saved.id, None)
        .await
        .unwrap();

    let updated = store.get(&saved.id).await.unwrap().unwrap();
    assert!(updated.next_instance_date.is_none());
}

#[tokio::test]
async fn test_update_next_instance_date_missing_task() {
    let (mut store, _dir) = create_store().await;
    let result = store
        .update_next_instance_date("nonexistent", Some(Utc::now()))
        .await;
    assert!(result.is_err());
}

// ─── Template persistence ──────────────────────────────────────

#[tokio::test]
async fn test_template_fields_persist() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("todos.jsonl");

    let template_id = {
        let mut store = TodoStore::new(file_path.clone());
        let mut t = create_test_todo("Weekly sync");
        t.is_template = true;
        t.recurrence_rule = Some("FREQ=WEEKLY;BYDAY=MO".to_string());
        t.next_instance_date = Some(Utc.with_ymd_and_hms(2025, 7, 7, 9, 0, 0).unwrap());
        let saved = store.add(t).await.unwrap();
        saved.id
    };

    // Reload
    let mut store2 = TodoStore::new(file_path);
    let loaded = store2.get(&template_id).await.unwrap().unwrap();
    assert!(loaded.is_template);
    assert_eq!(
        loaded.recurrence_rule.as_deref(),
        Some("FREQ=WEEKLY;BYDAY=MO")
    );
    assert!(loaded.next_instance_date.is_some());
}

// ─── Spawned instance fields ───────────────────────────────────

#[tokio::test]
async fn test_spawned_instance_links_to_template() {
    let (mut store, _dir) = create_store().await;

    let mut template = create_test_todo("Daily standup");
    template.is_template = true;
    template.recurrence_rule = Some("FREQ=DAILY".to_string());
    let template = store.add(template).await.unwrap();

    // Simulate spawning an instance
    let mut instance = Todo::default_instance();
    instance.title = "Daily standup".to_string();
    instance.recurrence_parent_id = Some(template.id.clone());
    let instance = store.add(instance).await.unwrap();

    assert_eq!(
        instance.recurrence_parent_id.as_deref(),
        Some(template.id.as_str())
    );
    assert!(!instance.is_template, "Instance should not be a template");
    assert!(
        instance.recurrence_rule.is_none(),
        "Instance should not have an RRULE"
    );
}

// ─── Cross-cutting: dependencies + templates ───────────────────

#[tokio::test]
async fn test_template_excluded_from_dependency_graph() {
    let (mut store, _dir) = create_store().await;

    // Template should not interfere with normal task dependency operations
    let mut template = create_test_todo("Weekly sync");
    template.is_template = true;
    template.recurrence_rule = Some("FREQ=WEEKLY".to_string());
    store.add(template).await.unwrap();

    let task_a = store.add(create_test_todo("Task A")).await.unwrap();
    let task_b = store.add(create_test_todo("Task B")).await.unwrap();

    // Dependencies between normal tasks should work fine
    store.add_dependency(&task_a.id, &task_b.id).await.unwrap();

    // Normal list excludes templates
    let normal = store.list(&TodoFilter::default()).await.unwrap();
    assert_eq!(
        normal.len(),
        2,
        "Templates should not appear in normal list"
    );

    // Dependency operations unaffected by templates
    let blockers = store.incomplete_blockers(&task_a.id).await.unwrap();
    assert_eq!(blockers.len(), 1);
    assert_eq!(blockers[0].id, task_b.id);
}

#[tokio::test]
async fn test_multiple_templates_with_different_rules() {
    let (mut store, _dir) = create_store().await;

    let mut daily = create_test_todo("Daily standup");
    daily.is_template = true;
    daily.recurrence_rule = Some("FREQ=DAILY".to_string());
    store.add(daily).await.unwrap();

    let mut weekly = create_test_todo("Weekly review");
    weekly.is_template = true;
    weekly.recurrence_rule = Some("FREQ=WEEKLY;BYDAY=FR".to_string());
    store.add(weekly).await.unwrap();

    let mut monthly = create_test_todo("Monthly report");
    monthly.is_template = true;
    monthly.recurrence_rule = Some("FREQ=MONTHLY;BYMONTHDAY=1".to_string());
    store.add(monthly).await.unwrap();

    let templates = store.list_templates().await.unwrap();
    assert_eq!(templates.len(), 3);

    // Normal list should exclude all 3
    let normal = store.list(&TodoFilter::default()).await.unwrap();
    assert!(normal.is_empty());
}

#[tokio::test]
async fn test_spawned_instance_can_have_dependencies() {
    let (mut store, _dir) = create_store().await;

    // Create a template
    let mut template = create_test_todo("Deploy");
    template.is_template = true;
    template.recurrence_rule = Some("FREQ=WEEKLY".to_string());
    let template = store.add(template).await.unwrap();

    // Spawn an instance (simulating what RecurringTaskSpawner does)
    let mut instance = Todo::default_instance();
    instance.title = "Deploy".to_string();
    instance.recurrence_parent_id = Some(template.id.clone());
    let instance = store.add(instance).await.unwrap();

    // Create a blocker for the instance
    let prereq = store.add(create_test_todo("Run tests")).await.unwrap();
    store
        .add_dependency(&instance.id, &prereq.id)
        .await
        .unwrap();

    // Instance should be blocked
    let blockers = store.incomplete_blockers(&instance.id).await.unwrap();
    assert_eq!(blockers.len(), 1);
    assert_eq!(blockers[0].title, "Run tests");
}

#[tokio::test]
async fn test_default_instance_has_clean_fields() {
    let instance = Todo::default_instance();

    assert!(!instance.is_template);
    assert!(instance.recurrence_rule.is_none());
    assert!(instance.recurrence_parent_id.is_none());
    assert!(instance.next_instance_date.is_none());
    assert!(instance.blocked_by.is_empty());
    assert!(instance.blocks.is_empty());
    assert!(instance.attachments.is_empty());
    assert!(instance.time_entries.is_empty());
    assert_eq!(instance.status, TodoStatus::Todo);
    assert!(!instance.id.is_empty(), "Should have a generated ID");
}

// ═══════════════════════════════════════════════════════════════
// TodoTool — recur, list_recurring, delete_recurring actions
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_tool_recur_creates_template() {
    let (tool, store, _dir, ctx) = create_tool_and_store().await;

    let params = json!({
        "action": "recur",
        "title": "Daily standup",
        "rule": "FREQ=DAILY"
    });

    let result = tool.execute(params, &ctx).await.unwrap();
    assert!(
        result.contains("Recurring task created"),
        "Result: {}",
        result
    );
    assert!(result.contains("Daily standup"));
    assert!(result.contains("Every day"));

    // Verify in store
    let mut s = store.write().await;
    let all = s
        .list(&TodoFilter {
            include_templates: true,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(all.len(), 1);
    assert!(all[0].is_template);
    assert_eq!(all[0].recurrence_rule.as_deref(), Some("FREQ=DAILY"));
    assert!(all[0].next_instance_date.is_some());
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_tool_recur_with_optional_fields() {
    let (tool, store, _dir, ctx) = create_tool_and_store().await;

    let params = json!({
        "action": "recur",
        "title": "Weekly review",
        "rule": "FREQ=WEEKLY;BYDAY=FR",
        "description": "End of week review",
        "priority": 3,
        "tags": ["meetings", "review"],
        "project_id": "proj-123"
    });

    let result = tool.execute(params, &ctx).await.unwrap();
    assert!(result.contains("Weekly review"));

    let mut s = store.write().await;
    let templates = s.list_templates().await.unwrap();
    let template = &templates[0];
    assert_eq!(template.description.as_deref(), Some("End of week review"));
    assert_eq!(template.priority, Some(3));
    assert_eq!(template.tags, vec!["meetings", "review"]);
    assert_eq!(template.project_id.as_deref(), Some("proj-123"));
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_tool_recur_rejects_invalid_rule() {
    let (tool, _store, _dir, ctx) = create_tool_and_store().await;

    let params = json!({
        "action": "recur",
        "title": "Bad rule",
        "rule": "FREQ=DAILY;COUNT=10"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err(), "COUNT should be rejected");
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("COUNT"), "Error: {}", err);
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_tool_recur_rejects_invalid_syntax() {
    let (tool, _store, _dir, ctx) = create_tool_and_store().await;

    let params = json!({
        "action": "recur",
        "title": "Bad",
        "rule": "NOT_VALID"
    });

    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_tool_list_recurring_empty() {
    let (tool, _store, _dir, ctx) = create_tool_and_store().await;

    let params = json!({ "action": "list_recurring" });
    let result = tool.execute(params, &ctx).await.unwrap();
    assert!(
        result.contains("No recurring"),
        "Should say no templates: {}",
        result
    );
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_tool_list_recurring_shows_templates() {
    let (tool, _store, _dir, ctx) = create_tool_and_store().await;

    // Create two templates
    tool.execute(
        json!({
            "action": "recur",
            "title": "Daily standup",
            "rule": "FREQ=DAILY"
        }),
        &ctx,
    )
    .await
    .unwrap();

    tool.execute(
        json!({
            "action": "recur",
            "title": "Weekly review",
            "rule": "FREQ=WEEKLY;BYDAY=MO",
            "priority": 2
        }),
        &ctx,
    )
    .await
    .unwrap();

    let params = json!({ "action": "list_recurring" });
    let result = tool.execute(params, &ctx).await.unwrap();

    assert!(result.contains("2 recurring"), "Result: {}", result);
    assert!(result.contains("Daily standup"));
    assert!(result.contains("Weekly review"));
    assert!(result.contains("Every day"));
    assert!(result.contains("Every week"));
    assert!(result.contains("Priority: 2"));
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_tool_delete_recurring_removes_template() {
    let (tool, store, _dir, ctx) = create_tool_and_store().await;

    // Create template
    tool.execute(
        json!({
            "action": "recur",
            "title": "Daily standup",
            "rule": "FREQ=DAILY"
        }),
        &ctx,
    )
    .await
    .unwrap();

    let template_id = {
        let mut s = store.write().await;
        let templates = s.list_templates().await.unwrap();
        templates[0].id.clone()
    };

    let params = json!({
        "action": "delete_recurring",
        "template_id": template_id
    });
    let result = tool.execute(params, &ctx).await.unwrap();

    assert!(result.contains("deleted"), "Result: {}", result);
    assert!(result.contains("Daily standup"));

    // Verify removed
    let mut s = store.write().await;
    let templates = s.list_templates().await.unwrap();
    assert!(templates.is_empty());
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_tool_delete_recurring_rejects_non_template() {
    let (tool, store, _dir, ctx) = create_tool_and_store().await;

    // Create a normal task (not a template)
    let task_id = {
        let mut s = store.write().await;
        let task = s.add(create_test_todo("Normal task")).await.unwrap();
        task.id
    };

    let params = json!({
        "action": "delete_recurring",
        "template_id": task_id
    });
    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err(), "Should reject non-template");
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("not a recurring template"), "Error: {}", err);
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_tool_delete_recurring_missing_template() {
    let (tool, _store, _dir, ctx) = create_tool_and_store().await;

    let params = json!({
        "action": "delete_recurring",
        "template_id": "nonexistent"
    });
    let result = tool.execute(params, &ctx).await;
    assert!(result.is_err(), "Should fail for missing template");
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("not found"), "Error: {}", err);
}
