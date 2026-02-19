//! Integration tests for I4: Goals in System Prompt
//!
//! AC-I4.1: ContextBuilder::new() accepts an optional goal_store parameter
//! AC-I4.2: get_goals_context() returns empty string when no active goals exist
//! AC-I4.3: get_goals_context() returns formatted string containing active goal titles
//! AC-I4.4: System prompt built by build_messages() includes goals when active goals exist
//! AC-I4.5: invalidate_goals_cache() forces re-fetch on next get_goals_context() call

use chrono::Utc;
use goal::{Goal, GoalStatus, GoalStore};
use klyntbot::agent::ContextBuilder;
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::RwLock;
use uuid::Uuid;

fn make_memory_note_repo() -> storage::MemoryNoteRepo {
    let pool = storage::StoragePool::connect_lazy("postgres://localhost/klyntbot_test").unwrap();
    storage::MemoryNoteRepo::new(pool.inner().clone())
}

fn make_goal(title: &str, status: GoalStatus) -> Goal {
    let now = Utc::now();
    Goal {
        id: Uuid::new_v4(),
        title: title.to_string(),
        description: format!("Test goal: {}", title),
        status,
        priority: 2,
        target_date: None,
        created_at: now,
        updated_at: now,
        metrics: vec![],
        linked_project_ids: vec![],
        metadata: HashMap::new(),
    }
}

/// AC-I4.1: ContextBuilder::new() accepts an optional goal_store parameter
#[tokio::test]
async fn test_context_builder_accepts_goal_store() {
    let tmp = TempDir::new().unwrap();
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let goal_store = Arc::new(RwLock::new(GoalStore::new(tmp.path().join("goals.jsonl"))));

    // Must compile and not panic — the 4th parameter is the goal_store
    let _ctx = ContextBuilder::new(
        workspace,
        "UTC".to_string(),
        None, // todo_store
        Some(goal_store),
        make_memory_note_repo(),
    )
    .await;
}

/// AC-I4.1 (no goal_store): ContextBuilder::new() also accepts None for goal_store
#[tokio::test]
async fn test_context_builder_accepts_no_goal_store() {
    let tmp = TempDir::new().unwrap();
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let _ctx = ContextBuilder::new(
        workspace,
        "UTC".to_string(),
        None, // todo_store
        None, // goal_store
        make_memory_note_repo(),
    )
    .await;
}

/// AC-I4.2: get_goals_context() returns empty string when there are no goals
#[tokio::test]
async fn test_goals_context_empty_when_no_goals() {
    let tmp = TempDir::new().unwrap();
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let goal_store = Arc::new(RwLock::new(GoalStore::new(tmp.path().join("goals.jsonl"))));

    let mut ctx = ContextBuilder::new(
        workspace,
        "UTC".to_string(),
        None,
        Some(goal_store),
        make_memory_note_repo(),
    )
    .await;

    let result = ctx.get_goals_context().await;
    assert!(
        result.is_empty(),
        "Expected empty goals context with no goals, got: {:?}",
        result
    );
}

/// AC-I4.3: get_goals_context() returns formatted string containing active goal titles
#[tokio::test]
async fn test_goals_context_includes_active_goals_only() {
    let tmp = TempDir::new().unwrap();
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let goal_store = Arc::new(RwLock::new(GoalStore::new(tmp.path().join("goals.jsonl"))));

    // Add an active and an abandoned goal
    {
        let mut store = goal_store.write().await;
        store
            .add(make_goal("Launch MVP", GoalStatus::Active))
            .await
            .unwrap();
        store
            .add(make_goal("Old Dream", GoalStatus::Abandoned))
            .await
            .unwrap();
    }

    let mut ctx = ContextBuilder::new(
        workspace,
        "UTC".to_string(),
        None,
        Some(goal_store),
        make_memory_note_repo(),
    )
    .await;

    let result = ctx.get_goals_context().await;
    assert!(
        !result.is_empty(),
        "Expected non-empty goals context with active goal"
    );
    assert!(
        result.contains("Launch MVP"),
        "Context should contain the active goal title, got: {:?}",
        result
    );
    assert!(
        !result.contains("Old Dream"),
        "Context should NOT contain abandoned goal, got: {:?}",
        result
    );
}

/// AC-I4.4: System prompt built by build_messages() includes goals section when active goals exist
#[tokio::test]
async fn test_system_prompt_includes_goals_section() {
    let tmp = TempDir::new().unwrap();
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let goal_store = Arc::new(RwLock::new(GoalStore::new(tmp.path().join("goals.jsonl"))));

    // Add an active goal
    {
        let mut store = goal_store.write().await;
        store
            .add(make_goal("Ship v2.0", GoalStatus::Active))
            .await
            .unwrap();
    }

    let mut ctx = ContextBuilder::new(
        workspace,
        "UTC".to_string(),
        None,
        Some(goal_store),
        make_memory_note_repo(),
    )
    .await;
    ctx.init().await.unwrap();

    let messages = ctx
        .build_messages(vec![], "hello", None, "test", "chat1")
        .await;

    // First message is always the system prompt
    assert!(!messages.is_empty(), "Expected at least one message");

    let system_content = match &messages[0] {
        klyntbot::Message::System { content } => content.clone(),
        _ => panic!("First message should be the system prompt"),
    };

    assert!(
        system_content.contains("Ship v2.0"),
        "System prompt should include active goal title 'Ship v2.0', got:\n{}",
        &system_content[..system_content.len().min(500)]
    );
}

/// AC-I4.5: invalidate_goals_cache() forces a re-fetch on the next get_goals_context() call
#[tokio::test]
async fn test_invalidate_goals_cache_refreshes_context() {
    let tmp = TempDir::new().unwrap();
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let goal_store = Arc::new(RwLock::new(GoalStore::new(tmp.path().join("goals.jsonl"))));

    let mut ctx = ContextBuilder::new(
        workspace,
        "UTC".to_string(),
        None,
        Some(goal_store.clone()),
        make_memory_note_repo(),
    )
    .await;

    // First call: no goals → empty result, cached
    let first = ctx.get_goals_context().await;
    assert!(
        first.is_empty(),
        "Expected empty context before any goals added"
    );

    // Add a goal AFTER the cache was populated (cache is warm with empty string)
    {
        let mut store = goal_store.write().await;
        store
            .add(make_goal("New Horizon", GoalStatus::Active))
            .await
            .unwrap();
    }

    // Invalidate so the next call re-reads from the store
    ctx.invalidate_goals_cache();

    let second = ctx.get_goals_context().await;
    assert!(
        second.contains("New Horizon"),
        "After invalidation, context should include the newly added goal, got: {:?}",
        second
    );
}
