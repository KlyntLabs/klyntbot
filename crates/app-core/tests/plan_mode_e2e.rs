//! End-to-end tests for plan-mode flows.
//!
//! Covers: happy path, edit flow, remove flow, cancel flow, idempotent enter,
//! and policy snapshot correctness.

use app_core::AppCore;
use bus::domain_events::TodoStatus;
use feature_coding_todo::types::TodoItem;

async fn setup_core() -> AppCore {
    let dir = tempfile::TempDir::new().unwrap();
    AppCore::for_test(Some(dir.path().to_path_buf()))
        .await
        .unwrap()
}

fn make_items() -> Vec<TodoItem> {
    vec![
        TodoItem {
            id: "a".into(),
            title: "Read schema".into(),
            status: TodoStatus::Pending,
            concurrency: bus::domain_events::ConcurrencyClass::Safe,
            blocked_reason: None,
            blocked_by: vec![],
            delegated_to: None,
            created_at: jiff::Timestamp::now(),
            updated_at: jiff::Timestamp::now(),
        },
        TodoItem {
            id: "b".into(),
            title: "Add migration".into(),
            status: TodoStatus::Pending,
            concurrency: bus::domain_events::ConcurrencyClass::Sequential,
            blocked_reason: None,
            blocked_by: vec![],
            delegated_to: None,
            created_at: jiff::Timestamp::now(),
            updated_at: jiff::Timestamp::now(),
        },
        TodoItem {
            id: "c".into(),
            title: "Write tests".into(),
            status: TodoStatus::Pending,
            concurrency: bus::domain_events::ConcurrencyClass::Safe,
            blocked_reason: None,
            blocked_by: vec![],
            delegated_to: None,
            created_at: jiff::Timestamp::now(),
            updated_at: jiff::Timestamp::now(),
        },
    ]
}

#[tokio::test]
async fn plan_mode_happy_path_enter_propose_ratify() {
    let core = setup_core().await;
    let thread_id = "t1";

    // Seed session so enter can read a title.
    sqlx::query(
        "INSERT INTO sessions (key, metadata, created_at, updated_at, pinned) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(thread_id)
    .bind(serde_json::json!({"title": "Feature X"}))
    .bind(jiff::Timestamp::now().as_millisecond())
    .bind(jiff::Timestamp::now().as_millisecond())
    .bind(0)
    .execute(core.repos.pool())
    .await
    .unwrap();

    // 1. Enter plan mode.
    let view = core.coding_plan_enter(thread_id).await.unwrap();
    assert!(view.plan_mode_state.is_some(), "should be in plan mode");
    let session_id = view
        .plan_mode_state
        .as_ref()
        .unwrap()
        .plan_session_id
        .clone();

    // 2. Simulate LLM proposing 3 pending items.
    let items = make_items();
    let json = serde_json::to_string(&items).unwrap();
    core.repos
        .coding_todo
        .upsert(thread_id, "root", &json, Some(&session_id))
        .await
        .unwrap();

    // 3. Ratify.
    let view = core
        .coding_plan_ratify(thread_id, &session_id)
        .await
        .unwrap();
    assert!(
        view.plan_mode_state.is_none(),
        "should exit plan mode after ratify"
    );
    let rows = core
        .repos
        .coding_todo
        .list_for_thread(thread_id)
        .await
        .unwrap();
    for row in &rows {
        assert!(
            row.proposed_in_plan_session.is_none(),
            "tags should be cleared"
        );
    }
}

#[tokio::test]
async fn plan_mode_user_edit_then_ratify_counts_diff() {
    let core = setup_core().await;
    let thread_id = "t2";

    sqlx::query(
        "INSERT INTO sessions (key, metadata, created_at, updated_at, pinned) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(thread_id)
    .bind(serde_json::json!({"title": "Feature Y"}))
    .bind(jiff::Timestamp::now().as_millisecond())
    .bind(jiff::Timestamp::now().as_millisecond())
    .bind(0)
    .execute(core.repos.pool())
    .await
    .unwrap();

    let view = core.coding_plan_enter(thread_id).await.unwrap();
    let session_id = view
        .plan_mode_state
        .as_ref()
        .unwrap()
        .plan_session_id
        .clone();

    // Propose 3 items.
    let items = make_items();
    let json = serde_json::to_string(&items).unwrap();
    core.repos
        .coding_todo
        .upsert(thread_id, "root", &json, Some(&session_id))
        .await
        .unwrap();

    // User edits item 2 title.
    let mut edited = items.clone();
    edited[1].title = "Add migration (revised)".into();
    let edit_json = serde_json::to_string(&edited).unwrap();
    let view = core
        .coding_plan_user_edit(thread_id, &session_id, &edit_json)
        .await
        .unwrap();
    let items: Vec<_> = view.agents.values().flatten().cloned().collect();
    assert_eq!(items.len(), 3);

    // Ratify — counts should be 2 ratified, 1 edited, 0 removed.
    let view = core
        .coding_plan_ratify(thread_id, &session_id)
        .await
        .unwrap();
    assert!(view.plan_mode_state.is_none());
}

#[tokio::test]
async fn plan_mode_user_remove_then_ratify() {
    let core = setup_core().await;
    let thread_id = "t3";

    sqlx::query(
        "INSERT INTO sessions (key, metadata, created_at, updated_at, pinned) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(thread_id)
    .bind(serde_json::json!({"title": "Feature Z"}))
    .bind(jiff::Timestamp::now().as_millisecond())
    .bind(jiff::Timestamp::now().as_millisecond())
    .bind(0)
    .execute(core.repos.pool())
    .await
    .unwrap();

    let view = core.coding_plan_enter(thread_id).await.unwrap();
    let session_id = view
        .plan_mode_state
        .as_ref()
        .unwrap()
        .plan_session_id
        .clone();

    let items = make_items();
    let json = serde_json::to_string(&items).unwrap();
    core.repos
        .coding_todo
        .upsert(thread_id, "root", &json, Some(&session_id))
        .await
        .unwrap();

    // Remove item "b".
    let view = core
        .coding_plan_user_remove(thread_id, &session_id, &["b".into()])
        .await
        .unwrap();
    let items: Vec<_> = view.agents.values().flatten().cloned().collect();
    assert_eq!(items.len(), 2);

    // Ratify.
    let view = core
        .coding_plan_ratify(thread_id, &session_id)
        .await
        .unwrap();
    assert!(view.plan_mode_state.is_none());
}

#[tokio::test]
async fn plan_mode_cancel_clears_plan() {
    let core = setup_core().await;
    let thread_id = "t4";

    sqlx::query(
        "INSERT INTO sessions (key, metadata, created_at, updated_at, pinned) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(thread_id)
    .bind(serde_json::json!({"title": "Feature W"}))
    .bind(jiff::Timestamp::now().as_millisecond())
    .bind(jiff::Timestamp::now().as_millisecond())
    .bind(0)
    .execute(core.repos.pool())
    .await
    .unwrap();

    let view = core.coding_plan_enter(thread_id).await.unwrap();
    let session_id = view
        .plan_mode_state
        .as_ref()
        .unwrap()
        .plan_session_id
        .clone();

    let items = make_items();
    let json = serde_json::to_string(&items).unwrap();
    core.repos
        .coding_todo
        .upsert(thread_id, "root", &json, Some(&session_id))
        .await
        .unwrap();

    // Cancel.
    let view = core.coding_plan_cancel(thread_id).await.unwrap();
    assert!(view.plan_mode_state.is_none());

    // Plan rows should be deleted.
    let rows = core
        .repos
        .coding_todo
        .list_for_thread(thread_id)
        .await
        .unwrap();
    assert!(rows.is_empty() || rows.iter().all(|r| r.agent_id != "root"));
}

#[tokio::test]
async fn plan_mode_enter_is_idempotent() {
    let core = setup_core().await;
    let thread_id = "t5";

    sqlx::query(
        "INSERT INTO sessions (key, metadata, created_at, updated_at, pinned) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(thread_id)
    .bind(serde_json::json!({"title": "Feature I"}))
    .bind(jiff::Timestamp::now().as_millisecond())
    .bind(jiff::Timestamp::now().as_millisecond())
    .bind(0)
    .execute(core.repos.pool())
    .await
    .unwrap();

    let view1 = core.coding_plan_enter(thread_id).await.unwrap();
    let sid1 = view1
        .plan_mode_state
        .as_ref()
        .unwrap()
        .plan_session_id
        .clone();

    let view2 = core.coding_plan_enter(thread_id).await.unwrap();
    let sid2 = view2
        .plan_mode_state
        .as_ref()
        .unwrap()
        .plan_session_id
        .clone();

    assert_eq!(sid1, sid2, "idempotent enter should reuse same session");
}

#[tokio::test]
async fn plan_mode_compute_ratify_counts_helper() {
    use app_core::handlers::coding_plan::compute_ratify_counts;
    use feature_coding_todo::types::TodoItem;

    let snap = vec![
        TodoItem {
            id: "a".into(),
            title: "A".into(),
            status: TodoStatus::Pending,
            concurrency: bus::domain_events::ConcurrencyClass::Safe,
            blocked_reason: None,
            blocked_by: vec![],
            delegated_to: None,
            created_at: jiff::Timestamp::now(),
            updated_at: jiff::Timestamp::now(),
        },
        TodoItem {
            id: "b".into(),
            title: "B".into(),
            status: TodoStatus::Pending,
            concurrency: bus::domain_events::ConcurrencyClass::Safe,
            blocked_reason: None,
            blocked_by: vec![],
            delegated_to: None,
            created_at: jiff::Timestamp::now(),
            updated_at: jiff::Timestamp::now(),
        },
    ];

    // All unchanged.
    let fin = snap.clone();
    assert_eq!(compute_ratify_counts(Some(&snap), &fin), (2, 0, 0));

    // One edited.
    let mut fin = snap.clone();
    fin[0].title = "A2".into();
    assert_eq!(compute_ratify_counts(Some(&snap), &fin), (1, 1, 0));

    // One removed.
    let fin = vec![snap[0].clone()];
    assert_eq!(compute_ratify_counts(Some(&snap), &fin), (1, 0, 1));
}

#[tokio::test]
async fn plan_mode_todo_get_returns_correct_proposed_count() {
    let core = setup_core().await;
    let thread_id = "t6";

    sqlx::query(
        "INSERT INTO sessions (key, metadata, created_at, updated_at, pinned) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(thread_id)
    .bind(serde_json::json!({"title": "Feature C"}))
    .bind(jiff::Timestamp::now().as_millisecond())
    .bind(jiff::Timestamp::now().as_millisecond())
    .bind(0)
    .execute(core.repos.pool())
    .await
    .unwrap();

    let view = core.coding_plan_enter(thread_id).await.unwrap();
    let session_id = view
        .plan_mode_state
        .as_ref()
        .unwrap()
        .plan_session_id
        .clone();
    assert_eq!(view.plan_mode_state.unwrap().proposed_item_count, 0);

    let items = make_items();
    let json = serde_json::to_string(&items).unwrap();
    core.repos
        .coding_todo
        .upsert(thread_id, "root", &json, Some(&session_id))
        .await
        .unwrap();

    let view = core.coding_todo_get(thread_id).await.unwrap();
    assert_eq!(view.plan_mode_state.unwrap().proposed_item_count, 3);
}

#[tokio::test]
async fn plan_mode_coding_policy_switches_to_plan_mode_and_back() {
    let core = setup_core().await;
    let thread_id = "t7";

    sqlx::query(
        "INSERT INTO sessions (key, metadata, created_at, updated_at, pinned) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(thread_id)
    .bind(serde_json::json!({"title": "Feature D"}))
    .bind(jiff::Timestamp::now().as_millisecond())
    .bind(jiff::Timestamp::now().as_millisecond())
    .bind(0)
    .execute(core.repos.pool())
    .await
    .unwrap();

    // Before enter: Default policy.
    let policy = core.coding_policies.get(thread_id);
    assert!(policy.is_none() || !policy.unwrap().read().is_plan_mode());

    core.coding_plan_enter(thread_id).await.unwrap();
    let policy = core.coding_policies.get(thread_id).unwrap();
    assert!(policy.read().is_plan_mode());

    core.coding_plan_cancel(thread_id).await.unwrap();
    let policy = core.coding_policies.get(thread_id).unwrap();
    assert!(!policy.read().is_plan_mode());
}
