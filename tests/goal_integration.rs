//! Integration tests for the Goal Engine (Phase 1)
//!
//! End-to-end tests verifying:
//! - Goal lifecycle: create → retrieve → update → delete
//! - GoalStore persistence across reloads
//! - GoalProgress calculation with metrics
//! - GoalSuggestionEngine intent detection
//! - GoalHandlerImpl delegation to GoalStore

use chrono::Utc;
use goal::{Goal, GoalStatus, GoalStore, GoalSuggestionEngine, Metric};
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::RwLock;

use agent::GoalHandlerImpl;
use tools::goal_tool::GoalHandler;
use uuid::Uuid;

fn create_goal(title: &str, priority: u8) -> Goal {
    let now = Utc::now();
    Goal {
        id: Uuid::new_v4(),
        title: title.to_string(),
        description: format!("Integration test goal: {}", title),
        status: GoalStatus::Active,
        priority,
        target_date: None,
        created_at: now,
        updated_at: now,
        metrics: vec![],
        linked_project_ids: vec![],
        metadata: HashMap::new(),
    }
}

#[tokio::test]
async fn test_goal_full_lifecycle() {
    let tmp = TempDir::new().unwrap();
    let store = Arc::new(RwLock::new(GoalStore::new(
        tmp.path().join("goals.jsonl"),
    )));
    let handler = GoalHandlerImpl::new(store);

    // Create
    let goal = create_goal("Launch MVP", 1);
    let id = handler.create_goal(goal).await.unwrap();

    // Retrieve
    let retrieved = handler.get_goal(&id).await.unwrap().unwrap();
    assert_eq!(retrieved.title, "Launch MVP");
    assert_eq!(retrieved.priority, 1);
    assert_eq!(retrieved.status, GoalStatus::Active);

    // Update
    let mut updated = retrieved;
    updated.title = "Launch MVP v2".to_string();
    updated.status = GoalStatus::Paused;
    handler.update_goal(updated).await.unwrap();

    let after_update = handler.get_goal(&id).await.unwrap().unwrap();
    assert_eq!(after_update.title, "Launch MVP v2");
    assert_eq!(after_update.status, GoalStatus::Paused);

    // List
    let all = handler.list_goals(None).await.unwrap();
    assert_eq!(all.len(), 1);

    let paused = handler.list_goals(Some(GoalStatus::Paused)).await.unwrap();
    assert_eq!(paused.len(), 1);

    let active = handler.list_goals(Some(GoalStatus::Active)).await.unwrap();
    assert_eq!(active.len(), 0);

    // Delete
    handler.delete_goal(&id).await.unwrap();
    let after_delete = handler.get_goal(&id).await.unwrap();
    assert!(after_delete.is_none());
}

#[tokio::test]
async fn test_goal_persistence_across_reloads() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("goals.jsonl");
    let goal_id;

    // First session: create goal
    {
        let store = Arc::new(RwLock::new(GoalStore::new(path.clone())));
        let handler = GoalHandlerImpl::new(store);

        let goal = create_goal("Persistent Goal", 2);
        goal_id = handler.create_goal(goal).await.unwrap();
    }

    // Second session: verify goal survived reload
    {
        let store = Arc::new(RwLock::new(GoalStore::new(path)));
        let handler = GoalHandlerImpl::new(store);

        let retrieved = handler.get_goal(&goal_id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title, "Persistent Goal");
    }
}

#[tokio::test]
async fn test_goal_progress_calculation() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("goals.jsonl");
    let mut store = GoalStore::new(path);

    let mut goal = create_goal("Track Progress", 2);
    goal.metrics = vec![
        Metric {
            name: "Features".to_string(),
            current: 3.0,
            target: 10.0,
            unit: "features".to_string(),
        },
        Metric {
            name: "Tests".to_string(),
            current: 20.0,
            target: 20.0,
            unit: "tests".to_string(),
        },
    ];
    let id = goal.id;
    store.add(goal).await.unwrap();

    let progress = store.calculate_progress(&id).await.unwrap().unwrap();
    assert_eq!(progress.goal_id, id);
    // (30% + 100%) / 2 = 65%
    assert!((progress.completion_percentage - 65.0).abs() < f64::EPSILON);
    assert_eq!(progress.metrics.len(), 2);
    assert!(progress.summary.contains("65"));
}

#[tokio::test]
async fn test_goal_suggestion_engine_integration() {
    let engine = GoalSuggestionEngine::new();

    // Should detect goal intent
    let suggestion = engine.detect_goal_intent("I want to build a mobile app").unwrap();
    assert!(!suggestion.proposed_title.is_empty());
    assert!(suggestion.confidence > 0.0);
    assert!(suggestion.linked_items.is_empty());

    // Should not trigger on non-goal messages
    let no_goal = engine.detect_goal_intent("fix typo in readme");
    assert!(no_goal.is_none());
}

#[tokio::test]
async fn test_multiple_goals_with_different_statuses() {
    let tmp = TempDir::new().unwrap();
    let store = Arc::new(RwLock::new(GoalStore::new(
        tmp.path().join("goals.jsonl"),
    )));
    let handler = GoalHandlerImpl::new(store);

    // Create goals with different priorities
    let g1 = create_goal("High Priority", 1);
    let g2 = create_goal("Medium Priority", 3);
    let g3 = create_goal("Low Priority", 5);

    let id1 = handler.create_goal(g1).await.unwrap();
    handler.create_goal(g2).await.unwrap();
    handler.create_goal(g3).await.unwrap();

    // Transition first goal to achieved
    let mut goal1 = handler.get_goal(&id1).await.unwrap().unwrap();
    goal1.status = GoalStatus::Achieved;
    handler.update_goal(goal1).await.unwrap();

    // Verify filtering
    let active = handler.list_goals(Some(GoalStatus::Active)).await.unwrap();
    assert_eq!(active.len(), 2);

    let achieved = handler.list_goals(Some(GoalStatus::Achieved)).await.unwrap();
    assert_eq!(achieved.len(), 1);
    assert_eq!(achieved[0].title, "High Priority");
}
