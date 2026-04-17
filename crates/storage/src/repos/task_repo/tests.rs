use super::*;

use crate::rows::task::{
    TaskDecompositionRow, TaskEstimationRow, TaskExecutionRow, TaskRow, TaskSuggestionRow,
};

/// Create all task-related tables needed for tests.
/// The tasks table is from the feature-tasks migration, not core migrations,
/// so we create them manually in tests.
async fn create_test_tables(db: &sqlx::SqlitePool) {
    // We need the areas table to exist (it's a core migration table, already present),
    // but we need to create all task-* tables.
    sqlx::query(
        r#"
            CREATE TABLE IF NOT EXISTS tasks (
                id                   TEXT PRIMARY KEY,
                title                TEXT NOT NULL,
                description          TEXT,
                area_id              TEXT NOT NULL REFERENCES areas(id) ON DELETE CASCADE,
                project_id           TEXT,
                key_result_id        TEXT,
                objective_id         TEXT,
                parent_id            TEXT REFERENCES tasks(id) ON DELETE SET NULL,
                status_label_id      TEXT,
                group_id             TEXT,
                priority             INTEGER,
                position             INTEGER NOT NULL DEFAULT 0,
                due_date             INTEGER,
                tags                 TEXT NOT NULL DEFAULT '[]',
                status               TEXT NOT NULL DEFAULT 'todo',
                task_type            TEXT NOT NULL DEFAULT 'manual',
                focused_at           INTEGER,
                focus_deadline       INTEGER,
                focus_expired_count  INTEGER NOT NULL DEFAULT 0,
                created_at           INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
                updated_at           INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
                completed_at         INTEGER,
                completed            INTEGER NOT NULL DEFAULT 0,
                total_tracked_secs   INTEGER NOT NULL DEFAULT 0,
                estimated_minutes    INTEGER,
                actual_minutes       INTEGER,
                calendar_event_uid   TEXT,
                last_reminded_at     INTEGER,
                recurrence_rule      TEXT,
                recurrence_parent_id TEXT,
                is_template          INTEGER NOT NULL DEFAULT 0,
                next_instance_date   INTEGER,
                acceptance_criteria  TEXT,
                agent_config         TEXT,
                execution_state      TEXT NOT NULL DEFAULT 'idle',
                spawned_execution_id TEXT,
                context_snapshot     TEXT,
                energy_level         TEXT DEFAULT 'medium',
                estimated_focus_blocks INTEGER,
                complexity_score     INTEGER,
                scheduled_start      INTEGER,
                scheduled_end        INTEGER
            )
            "#,
    )
    .execute(db)
    .await
    .unwrap();

    sqlx::query(
        r#"
            CREATE TABLE IF NOT EXISTS task_activity (
                id            TEXT PRIMARY KEY,
                task_id       TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                activity_type TEXT NOT NULL,
                field_changed TEXT,
                old_value     TEXT,
                new_value     TEXT,
                actor_type    TEXT NOT NULL DEFAULT 'user',
                actor_id      TEXT,
                summary       TEXT,
                created_at    INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000)
            )
            "#,
    )
    .execute(db)
    .await
    .unwrap();

    sqlx::query(
        r#"
            CREATE TABLE IF NOT EXISTS task_executions (
                id             TEXT PRIMARY KEY,
                task_id        TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                status         TEXT NOT NULL DEFAULT 'pending',
                agent_profile  TEXT,
                started_at     INTEGER,
                completed_at   INTEGER,
                duration_secs  INTEGER,
                tokens_used    INTEGER,
                cost_usd       REAL,
                input_context  TEXT,
                output_summary TEXT,
                error_message  TEXT,
                artifacts      TEXT,
                metrics        TEXT,
                retry_count    INTEGER NOT NULL DEFAULT 0,
                created_at     INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000)
            )
            "#,
    )
    .execute(db)
    .await
    .unwrap();

    sqlx::query(
        r#"
            CREATE TABLE IF NOT EXISTS task_suggestions (
                id              TEXT PRIMARY KEY,
                task_id         TEXT REFERENCES tasks(id) ON DELETE CASCADE,
                suggestion_type TEXT NOT NULL,
                title           TEXT NOT NULL,
                description     TEXT,
                confidence      REAL NOT NULL DEFAULT 0.0,
                action_payload  TEXT,
                status          TEXT NOT NULL DEFAULT 'pending',
                trigger         TEXT,
                created_at      INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
                resolved_at     INTEGER
            )
            "#,
    )
    .execute(db)
    .await
    .unwrap();

    sqlx::query(
        r#"
            CREATE TABLE IF NOT EXISTS task_attachments (
                id              TEXT PRIMARY KEY,
                task_id         TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                attachment_type TEXT NOT NULL,
                value           TEXT NOT NULL,
                title           TEXT,
                tags            TEXT NOT NULL DEFAULT '[]',
                source          TEXT NOT NULL DEFAULT 'user',
                created_at      INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000)
            )
            "#,
    )
    .execute(db)
    .await
    .unwrap();

    sqlx::query(
        r#"
            CREATE TABLE IF NOT EXISTS task_time_entries (
                id            TEXT PRIMARY KEY,
                task_id       TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                source        TEXT NOT NULL DEFAULT 'focus',
                started_at    INTEGER NOT NULL,
                ended_at      INTEGER,
                duration_secs INTEGER,
                energy_level  TEXT,
                note          TEXT
            )
            "#,
    )
    .execute(db)
    .await
    .unwrap();

    sqlx::query(
        r#"
            CREATE TABLE IF NOT EXISTS task_dependencies (
                task_id    TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                blocker_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                dep_type   TEXT NOT NULL DEFAULT 'blocks',
                PRIMARY KEY (task_id, blocker_id),
                CHECK (task_id != blocker_id)
            )
            "#,
    )
    .execute(db)
    .await
    .unwrap();

    sqlx::query(
        r#"
            CREATE TABLE IF NOT EXISTS task_estimation_history (
                id                TEXT PRIMARY KEY,
                task_id           TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                estimated_minutes INTEGER NOT NULL,
                actual_minutes    INTEGER NOT NULL,
                deviation_pct     REAL NOT NULL DEFAULT 0.0,
                complexity_score  INTEGER,
                energy_level      TEXT,
                tags              TEXT NOT NULL DEFAULT '[]',
                project_id        TEXT,
                completed_at      INTEGER NOT NULL
            )
            "#,
    )
    .execute(db)
    .await
    .unwrap();

    sqlx::query(
        r#"
            CREATE TABLE IF NOT EXISTS task_decompositions (
                id         TEXT PRIMARY KEY,
                task_id    TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                plan       TEXT NOT NULL,
                confidence REAL NOT NULL DEFAULT 0.0,
                status     TEXT NOT NULL DEFAULT 'pending',
                reasoning  TEXT,
                created_at INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
                applied_at INTEGER
            )
            "#,
    )
    .execute(db)
    .await
    .unwrap();

    // Insert a test area for FK references.
    sqlx::query(
        "INSERT OR IGNORE INTO areas (id, name, color, status) VALUES ('test-area', 'Test Area', '#000', 'active')",
    )
    .execute(db)
    .await
    .unwrap();
}

/// Helper to create a ready-to-use TaskRepo backed by an in-memory DB.
async fn setup_repo() -> TaskRepo {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let db = pool.inner().clone();
    create_test_tables(&db).await;
    TaskRepo::new(db)
}

/// Helper to create a minimal TaskRow for tests.
fn make_task(id: &str, title: &str) -> TaskRow {
    let now: crate::sqlite_types::SqlTs = jiff::Timestamp::now().into();
    TaskRow {
        id: id.to_string(),
        title: title.to_string(),
        description: None,
        area_id: "test-area".to_string(),
        project_id: None,
        key_result_id: None,
        parent_id: None,
        priority: None,
        due_date: None,
        tags: vec![],
        status: "todo".to_string(),
        focused_at: None,
        focus_deadline: None,
        focus_expired_count: 0,
        created_at: now,
        updated_at: now,
        completed_at: None,
        total_tracked_secs: 0,
        estimated_minutes: None,
        calendar_event_uid: None,
        last_reminded_at: None,
        recurrence_rule: None,
        recurrence_parent_id: None,
        is_template: false,
        next_instance_date: None,
        status_label_id: None,
        position: 0,
        group_id: None,
        task_type: "manual".to_string(),
        acceptance_criteria: None,
        agent_config: None,
        execution_state: "idle".to_string(),
        spawned_execution_id: None,
        context_snapshot: None,
        energy_level: Some("medium".to_string()),
        estimated_focus_blocks: None,
        actual_minutes: None,
        complexity_score: None,
        completed: false,
        objective_id: None,
        scheduled_start: None,
        scheduled_end: None,
    }
}

#[tokio::test]
async fn test_add_and_get() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let db = pool.inner().clone();
    create_test_tables(&db).await;
    let repo = TaskRepo::new(db);

    let task = make_task("t1", "Buy groceries");
    let inserted = repo.add(&task).await.unwrap();
    assert_eq!(inserted.id, "t1");
    assert_eq!(inserted.title, "Buy groceries");
    assert_eq!(inserted.task_type, "manual");
    assert_eq!(inserted.execution_state, "idle");
    assert!(!inserted.completed);

    // get
    let fetched = repo.get("t1").await.unwrap().unwrap();
    assert_eq!(fetched.title, "Buy groceries");

    // get_or_err
    let fetched2 = repo.get_or_err("t1").await.unwrap();
    assert_eq!(fetched2.id, "t1");

    // get_or_err on missing
    let err = repo.get_or_err("nonexistent").await;
    assert!(matches!(err, Err(crate::error::StorageError::NotFound(_))));

    // get_by_ids
    let task2 = make_task("t2", "Walk the dog");
    repo.add(&task2).await.unwrap();
    let batch = repo
        .get_by_ids(&["t1".into(), "t2".into(), "missing".into()])
        .await
        .unwrap();
    assert_eq!(batch.len(), 2);

    // get_by_ids with empty input
    let empty = repo.get_by_ids(&[]).await.unwrap();
    assert!(empty.is_empty());
}

#[tokio::test]
async fn test_update_task() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let db = pool.inner().clone();
    create_test_tables(&db).await;
    let repo = TaskRepo::new(db);

    let task = make_task("u1", "Original title");
    repo.add(&task).await.unwrap();

    // Update title and new agentic fields.
    let patch = TaskPatch {
        id: "u1".to_string(),
        title: Some("Updated title".to_string()),
        task_type: Some("agent_delegated".to_string()),
        execution_state: Some("running".to_string()),
        energy_level: Some(Some("high".to_string())),
        complexity_score: Some(Some(7)),
        completed: Some(true),
        ..Default::default()
    };
    let updated = repo.update(&patch).await.unwrap();
    assert_eq!(updated.title, "Updated title");
    assert_eq!(updated.task_type, "agent_delegated");
    assert_eq!(updated.execution_state, "running");
    assert_eq!(updated.energy_level.as_deref(), Some("high"));
    assert_eq!(updated.complexity_score, Some(7));
    assert!(updated.completed);

    // Update non-existent task.
    let bad_patch = TaskPatch {
        id: "nonexistent".to_string(),
        title: Some("Nope".to_string()),
        ..Default::default()
    };
    let err = repo.update(&bad_patch).await;
    assert!(matches!(err, Err(crate::error::StorageError::NotFound(_))));
}

#[tokio::test]
async fn test_delete_task() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let db = pool.inner().clone();
    create_test_tables(&db).await;
    let repo = TaskRepo::new(db);

    let task = make_task("d1", "Delete me");
    repo.add(&task).await.unwrap();

    let deleted = repo.delete("d1").await.unwrap();
    assert!(deleted);

    let gone = repo.get("d1").await.unwrap();
    assert!(gone.is_none());

    // Delete non-existent
    let not_deleted = repo.delete("d1").await.unwrap();
    assert!(!not_deleted);
}

#[tokio::test]
async fn test_list_with_filters() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let db = pool.inner().clone();
    create_test_tables(&db).await;
    let repo = TaskRepo::new(db);

    let mut t1 = make_task("f1", "Manual task");
    t1.task_type = "manual".to_string();
    t1.completed = false;
    repo.add(&t1).await.unwrap();

    let mut t2 = make_task("f2", "Agent task");
    t2.task_type = "agent_delegated".to_string();
    t2.completed = true;
    repo.add(&t2).await.unwrap();

    let mut t3 = make_task("f3", "Another manual");
    t3.task_type = "manual".to_string();
    t3.completed = true;
    repo.add(&t3).await.unwrap();

    // Filter by task_type
    let results = repo
        .list(&TaskFilter {
            task_type: Some("agent_delegated".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "f2");

    // Filter by completed
    let results = repo
        .list(&TaskFilter {
            completed: Some(true),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(results.len(), 2);

    let results = repo
        .list(&TaskFilter {
            completed: Some(false),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "f1");

    // Filter by energy_level
    let results = repo
        .list(&TaskFilter {
            energy_level: Some("medium".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(results.len(), 3);

    // Search (SQLite LIKE is case-insensitive for ASCII)
    let results = repo.search_by_keyword("manual", None).await.unwrap();
    assert_eq!(results.len(), 2);

    let results = repo.search_by_keyword("Agent", None).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "f2");
}

#[tokio::test]
async fn test_focus_unfocus() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let db = pool.inner().clone();
    create_test_tables(&db).await;
    let repo = TaskRepo::new(db);

    let t1 = make_task("fo1", "Focus me");
    let t2 = make_task("fo2", "Also focus me");
    repo.add(&t1).await.unwrap();
    repo.add(&t2).await.unwrap();

    // Focus first task
    let focused = repo.focus("fo1", 2, None).await.unwrap();
    assert!(focused);

    let list = repo.list_focused().await.unwrap();
    assert_eq!(list.len(), 1);

    // Focus second task
    let focused = repo.focus("fo2", 2, None).await.unwrap();
    assert!(focused);

    // Can't focus third (max_slots = 2)
    let t3 = make_task("fo3", "No room");
    repo.add(&t3).await.unwrap();
    let focused = repo.focus("fo3", 2, None).await.unwrap();
    assert!(!focused);

    // Unfocus
    let unfocused = repo.unfocus("fo1").await.unwrap();
    assert!(unfocused);

    let list = repo.list_focused().await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, "fo2");
}

#[tokio::test]
async fn test_log_activity() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let db = pool.inner().clone();
    create_test_tables(&db).await;
    let repo = TaskRepo::new(db);

    let task = make_task("act1", "Activity test");
    repo.add(&task).await.unwrap();

    let entry = repo
        .log_activity(
            "act1",
            "field_change",
            Some("status"),
            Some("todo"),
            Some("doing"),
            "user",
            Some("Status changed"),
        )
        .await
        .unwrap();
    assert_eq!(entry.task_id, "act1");
    assert_eq!(entry.activity_type, "field_change");
    assert_eq!(entry.field_changed.as_deref(), Some("status"));

    let entry2 = repo
        .log_activity(
            "act1",
            "comment",
            None,
            None,
            None,
            "agent",
            Some("Looks good"),
        )
        .await
        .unwrap();
    assert_eq!(entry2.actor_type, "agent");

    let entries = repo.list_activity("act1", 10).await.unwrap();
    assert_eq!(entries.len(), 2);
    let types: Vec<&str> = entries.iter().map(|e| e.activity_type.as_str()).collect();
    assert!(types.contains(&"field_change"));
    assert!(types.contains(&"comment"));
}

#[tokio::test]
async fn test_children_and_subtree() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let db = pool.inner().clone();
    create_test_tables(&db).await;
    let repo = TaskRepo::new(db);

    let parent = make_task("p1", "Parent");
    repo.add(&parent).await.unwrap();

    let mut child1 = make_task("c1", "Child 1");
    child1.parent_id = Some("p1".to_string());
    child1.completed = true;
    repo.add(&child1).await.unwrap();

    let mut child2 = make_task("c2", "Child 2");
    child2.parent_id = Some("p1".to_string());
    repo.add(&child2).await.unwrap();

    let mut grandchild = make_task("gc1", "Grandchild");
    grandchild.parent_id = Some("c1".to_string());
    repo.add(&grandchild).await.unwrap();

    // get_children
    let children = repo.get_children("p1").await.unwrap();
    assert_eq!(children.len(), 2);

    // count_children
    let (total, completed) = repo.count_children("p1").await.unwrap();
    assert_eq!(total, 2);
    assert_eq!(completed, 1);

    // count_children_bulk
    let bulk = repo
        .count_children_bulk(&["p1".into(), "c1".into()])
        .await
        .unwrap();
    assert_eq!(bulk.get("p1"), Some(&(2, 1)));
    assert_eq!(bulk.get("c1"), Some(&(1, 0)));

    // get_subtree
    let subtree = repo.get_subtree("p1").await.unwrap();
    assert_eq!(subtree.len(), 4); // parent + 2 children + grandchild

    // cascade_complete
    let completed_count = repo.cascade_complete("p1").await.unwrap();
    // parent, child2, grandchild were not completed (child1 was already completed)
    assert_eq!(completed_count, 3);

    let all = repo.list(&TaskFilter::default()).await.unwrap();
    assert!(all.iter().all(|t| t.completed));

    // move_task
    let moved = repo.move_task("gc1", Some("p1"), None).await.unwrap();
    assert_eq!(moved.parent_id.as_deref(), Some("p1"));

    // move_task cycle detection
    let err = repo.move_task("p1", Some("gc1"), None).await;
    assert!(matches!(err, Err(crate::error::StorageError::Conflict(_))));
}

#[tokio::test]
async fn test_dependencies() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let db = pool.inner().clone();
    create_test_tables(&db).await;
    let repo = TaskRepo::new(db);

    let t1 = make_task("dep1", "Task 1");
    let t2 = make_task("dep2", "Task 2");
    let t3 = make_task("dep3", "Task 3");
    repo.add(&t1).await.unwrap();
    repo.add(&t2).await.unwrap();
    repo.add(&t3).await.unwrap();

    // dep1 is blocked by dep2
    repo.add_dependency("dep1", "dep2", "blocks").await.unwrap();

    let blockers = repo.get_blockers("dep1").await.unwrap();
    assert_eq!(blockers.len(), 1);
    assert_eq!(blockers[0].id, "dep2");

    let blocking = repo.get_blocking("dep2").await.unwrap();
    assert_eq!(blocking.len(), 1);
    assert_eq!(blocking[0].id, "dep1");

    // incomplete_blockers
    let incomplete = repo.incomplete_blockers("dep1").await.unwrap();
    assert_eq!(incomplete.len(), 1);

    // Cycle detection: dep2 -> dep3 -> dep1 would create a cycle since dep1 -> dep2 exists
    repo.add_dependency("dep2", "dep3", "blocks").await.unwrap();
    let err = repo.add_dependency("dep3", "dep1", "blocks").await;
    assert!(matches!(err, Err(crate::error::StorageError::Conflict(_))));

    // Remove dependency
    let removed = repo.remove_dependency("dep1", "dep2").await.unwrap();
    assert!(removed);

    let blockers = repo.get_blockers("dep1").await.unwrap();
    assert!(blockers.is_empty());
}

#[tokio::test]
async fn test_attachments() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let db = pool.inner().clone();
    create_test_tables(&db).await;
    let repo = TaskRepo::new(db);

    let task = make_task("att1", "Attach test");
    repo.add(&task).await.unwrap();

    let att = repo
        .add_attachment(
            "att1",
            "link",
            "https://example.com",
            Some("Example"),
            &["ref".to_string()],
            "user",
        )
        .await
        .unwrap();
    assert_eq!(att.task_id, "att1");
    assert_eq!(att.source, "user");

    let list = repo.list_attachments("att1").await.unwrap();
    assert_eq!(list.len(), 1);

    let removed = repo.remove_attachment("att1", att.id).await.unwrap();
    assert!(removed);

    let list = repo.list_attachments("att1").await.unwrap();
    assert!(list.is_empty());
}

#[tokio::test]
async fn test_time_entries() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let db = pool.inner().clone();
    create_test_tables(&db).await;
    let repo = TaskRepo::new(db);

    let task = make_task("te1", "Time test");
    repo.add(&task).await.unwrap();

    let now = jiff::Timestamp::now();
    let entry = repo
        .add_time_entry(
            "te1",
            "manual",
            now,
            Some(3600),
            Some("Did some work"),
            Some("high"),
        )
        .await
        .unwrap();
    assert_eq!(entry.task_id, "te1");
    assert_eq!(entry.duration_secs, Some(3600));
    assert_eq!(entry.energy_level.as_deref(), Some("high"));

    // Check total_tracked_secs updated
    let updated_task = repo.get_or_err("te1").await.unwrap();
    assert_eq!(updated_task.total_tracked_secs, 3600);

    let list = repo.list_time_entries("te1").await.unwrap();
    assert_eq!(list.len(), 1);
}

#[tokio::test]
async fn test_executions() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let db = pool.inner().clone();
    create_test_tables(&db).await;
    let repo = TaskRepo::new(db);

    let task = make_task("ex1", "Exec test");
    repo.add(&task).await.unwrap();

    let now: crate::sqlite_types::SqlTs = jiff::Timestamp::now().into();
    let exec_row = TaskExecutionRow {
        id: "exec-1".to_string(),
        task_id: "ex1".to_string(),
        status: "pending".to_string(),
        agent_profile: Some("general".to_string()),
        started_at: Some(now),
        completed_at: None,
        duration_secs: None,
        tokens_used: None,
        cost_usd: None,
        input_context: Some("test context".to_string()),
        output_summary: None,
        error_message: None,
        artifacts: None,
        metrics: None,
        retry_count: 0,
        created_at: now,
    };
    let created = repo.create_execution(&exec_row).await.unwrap();
    assert_eq!(created.id, "exec-1");
    assert_eq!(created.status, "pending");

    let updated = repo
        .update_execution("exec-1", "completed", Some("All done"), None, None)
        .await
        .unwrap();
    assert_eq!(updated.status, "completed");
    assert_eq!(updated.output_summary.as_deref(), Some("All done"));
    assert!(updated.completed_at.is_some());

    let list = repo.list_executions("ex1").await.unwrap();
    assert_eq!(list.len(), 1);
}

#[tokio::test]
async fn test_suggestions() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let db = pool.inner().clone();
    create_test_tables(&db).await;
    let repo = TaskRepo::new(db);

    let task = make_task("sug1", "Suggestion test");
    repo.add(&task).await.unwrap();

    let now: crate::sqlite_types::SqlTs = jiff::Timestamp::now().into();
    let suggestion = TaskSuggestionRow {
        id: "s1".to_string(),
        task_id: Some("sug1".to_string()),
        suggestion_type: "decompose".to_string(),
        title: "Break this into subtasks".to_string(),
        description: None,
        confidence: 0.85,
        action_payload: None,
        status: "pending".to_string(),
        trigger: Some("complexity".to_string()),
        created_at: now,
        resolved_at: None,
    };
    let created = repo.create_suggestion(&suggestion).await.unwrap();
    assert_eq!(created.status, "pending");

    let pending = repo.list_pending_suggestions(Some("sug1")).await.unwrap();
    assert_eq!(pending.len(), 1);

    let resolved = repo.resolve_suggestion("s1", "accepted").await.unwrap();
    assert!(resolved);

    let pending = repo.list_pending_suggestions(Some("sug1")).await.unwrap();
    assert!(pending.is_empty());
}

#[tokio::test]
async fn test_estimation_stats() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let db = pool.inner().clone();
    create_test_tables(&db).await;
    let repo = TaskRepo::new(db);

    let task = make_task("est1", "Estimation test");
    repo.add(&task).await.unwrap();

    let now: crate::sqlite_types::SqlTs = jiff::Timestamp::now().into();
    let est1 = TaskEstimationRow {
        id: "e1".to_string(),
        task_id: "est1".to_string(),
        estimated_minutes: 60,
        actual_minutes: 90,
        deviation_pct: 50.0,
        complexity_score: Some(5),
        energy_level: Some("medium".to_string()),
        tags: vec!["coding".to_string()],
        project_id: None,
        completed_at: now,
    };
    repo.record_estimation(&est1).await.unwrap();

    let est2 = TaskEstimationRow {
        id: "e2".to_string(),
        task_id: "est1".to_string(),
        estimated_minutes: 30,
        actual_minutes: 30,
        deviation_pct: 0.0,
        complexity_score: Some(3),
        energy_level: Some("low".to_string()),
        tags: vec!["review".to_string()],
        project_id: None,
        completed_at: now,
    };
    repo.record_estimation(&est2).await.unwrap();

    // Overall stats
    let (avg, count) = repo.estimation_stats(None).await.unwrap();
    assert_eq!(count, 2);
    assert!((avg - 25.0).abs() < 0.01); // (50.0 + 0.0) / 2

    // Filter by tag
    let (avg, count) = repo
        .estimation_stats(Some(&["coding".to_string()]))
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert!((avg - 50.0).abs() < 0.01);
}

#[tokio::test]
async fn test_summary_and_overdue() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let db = pool.inner().clone();
    create_test_tables(&db).await;
    let repo = TaskRepo::new(db);

    let t1 = make_task("sum1", "Todo task");
    repo.add(&t1).await.unwrap();

    let mut t2 = make_task("sum2", "Doing task");
    t2.status = "doing".to_string();
    repo.add(&t2).await.unwrap();

    let mut t3 = make_task("sum3", "Done task");
    t3.status = "done".to_string();
    t3.completed = true;
    repo.add(&t3).await.unwrap();

    let summary = repo.summary().await.unwrap();
    assert_eq!(summary.todo, 1);
    assert_eq!(summary.doing, 1);
    assert_eq!(summary.done, 1);
    assert_eq!(summary.total, 3);

    // Overdue: task with past due date and not completed
    let mut overdue_task = make_task("sum4", "Overdue task");
    overdue_task.due_date = Some(crate::sqlite_types::SqlTs::from(
        jiff::Timestamp::now() - jiff::SignedDuration::from_hours(24),
    ));
    repo.add(&overdue_task).await.unwrap();

    let overdue = repo.overdue().await.unwrap();
    assert_eq!(overdue.len(), 1);
    assert_eq!(overdue[0].id, "sum4");
}

#[tokio::test]
async fn test_get_execution() {
    let repo = setup_repo().await;

    let task = make_task("exec-get-t", "Exec test");
    repo.add(&task).await.unwrap();

    let exec_row = TaskExecutionRow {
        id: "exec-1".to_string(),
        task_id: task.id.clone(),
        status: "pending".to_string(),
        agent_profile: Some("task".to_string()),
        started_at: None,
        completed_at: None,
        duration_secs: None,
        tokens_used: None,
        cost_usd: None,
        input_context: None,
        output_summary: None,
        error_message: None,
        artifacts: None,
        metrics: None,
        retry_count: 0,
        created_at: jiff::Timestamp::now().into(),
    };
    repo.create_execution(&exec_row).await.unwrap();

    let fetched = repo.get_execution("exec-1").await.unwrap();
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().status, "pending");

    let missing = repo.get_execution("nonexistent").await.unwrap();
    assert!(missing.is_none());
}

#[tokio::test]
async fn test_decomposition_crud() {
    let repo = setup_repo().await;

    let task = make_task("decomp-t", "Decomp test");
    repo.add(&task).await.unwrap();

    let row = TaskDecompositionRow {
        id: "decomp-1".to_string(),
        task_id: task.id.clone(),
        plan: r#"{"subtasks":[]}"#.to_string(),
        confidence: 0.85,
        status: "pending".to_string(),
        reasoning: Some("test".to_string()),
        created_at: jiff::Timestamp::now().into(),
        applied_at: None,
    };
    let created = repo.create_decomposition(&row).await.unwrap();
    assert_eq!(created.id, "decomp-1");

    let fetched = repo.get_decomposition("decomp-1").await.unwrap();
    assert!(fetched.is_some());

    let pending = repo.list_pending_decompositions(&task.id).await.unwrap();
    assert_eq!(pending.len(), 1);

    repo.apply_decomposition("decomp-1").await.unwrap();
    let applied = repo.get_decomposition("decomp-1").await.unwrap().unwrap();
    assert_eq!(applied.status, "applied");

    let pending_after = repo.list_pending_decompositions(&task.id).await.unwrap();
    assert!(pending_after.is_empty());
}

#[tokio::test]
async fn test_expire_suggestions() {
    let repo = setup_repo().await;

    let task = make_task("expire-t", "Expire test");
    repo.add(&task).await.unwrap();

    let sugg = TaskSuggestionRow {
        id: "sugg-1".to_string(),
        task_id: Some(task.id.clone()),
        suggestion_type: "Decompose".to_string(),
        title: "Break down".to_string(),
        description: None,
        confidence: 0.8,
        action_payload: None,
        status: "pending".to_string(),
        trigger: None,
        created_at: jiff::Timestamp::now().into(),
        resolved_at: None,
    };
    repo.create_suggestion(&sugg).await.unwrap();

    let expired = repo.expire_suggestions_for_task(&task.id).await.unwrap();
    assert_eq!(expired, 1);

    let pending = repo.list_pending_suggestions(Some(&task.id)).await.unwrap();
    assert!(pending.is_empty());
}
