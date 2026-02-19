//! Sprint 6 Integration Tests — Daily Planning Skill
//!
//! Tests the daily planning feature end-to-end: scoring algorithm, plan generation,
//! response parsing, cron integration, CLI commands, and config toggle.

use chrono::{Duration, Utc};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::RwLock;
use tools::{
    todo_store::TodoStore,
    todo_types::{Todo, TodoStatus},
};

// ─── Test Fixtures ─────────────────────────────────────────────

/// Creates a standard set of todos with varied urgency, priority, and age
/// for testing the planning engine's scoring and ranking behavior.
///
/// Returns: (store, temp_dir, task_ids) where task_ids maps:
///   "overdue_p1"   → high urgency, high priority (should rank #1)
///   "today_p2"     → due today, medium priority (should rank #2)
///   "tomorrow_p3"  → due tomorrow, low priority (should rank #3)
///   "future_p4"    → due in 7 days (should NOT be in top 3)
///   "completed"    → done task (should be excluded)
///   "template"     → recurring template (should be excluded)
///   "no_priority"  → no priority set, overdue (tests default priority handling)
async fn create_planning_test_data() -> (
    Arc<RwLock<TodoStore>>,
    TempDir,
    std::collections::HashMap<String, String>,
) {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("todos.jsonl");
    let store = Arc::new(RwLock::new(TodoStore::new(file_path)));
    let mut ids = std::collections::HashMap::new();

    let now = Utc::now();

    // Overdue P1 task — 2 days past due
    let mut t1 = Todo::default_instance();
    t1.title = "Fix auth token expiry bug".to_string();
    t1.priority = Some(1);
    t1.due_date = Some(now - Duration::days(2));
    t1.created_at = now - Duration::days(5);
    let t1 = store.write().await.add(t1).await.unwrap();
    ids.insert("overdue_p1".to_string(), t1.id.clone());

    // Due today P2 task
    let mut t2 = Todo::default_instance();
    t2.title = "Implement user settings page".to_string();
    t2.priority = Some(2);
    t2.due_date = Some(now);
    t2.created_at = now - Duration::days(3);
    let t2 = store.write().await.add(t2).await.unwrap();
    ids.insert("today_p2".to_string(), t2.id.clone());

    // Due tomorrow P3 task
    let mut t3 = Todo::default_instance();
    t3.title = "Update API docs for v2 endpoints".to_string();
    t3.priority = Some(3);
    t3.due_date = Some(now + Duration::days(1));
    t3.created_at = now - Duration::days(1);
    let t3 = store.write().await.add(t3).await.unwrap();
    ids.insert("tomorrow_p3".to_string(), t3.id.clone());

    // Future P4 task — should NOT be in top 3
    let mut t4 = Todo::default_instance();
    t4.title = "Refactor database module".to_string();
    t4.priority = Some(4);
    t4.due_date = Some(now + Duration::days(7));
    t4.created_at = now;
    let t4 = store.write().await.add(t4).await.unwrap();
    ids.insert("future_p4".to_string(), t4.id.clone());

    // Completed task — should be excluded from plan
    let mut t5 = Todo::default_instance();
    t5.title = "Already done task".to_string();
    t5.priority = Some(1);
    t5.due_date = Some(now - Duration::days(1));
    t5.status = TodoStatus::Done;
    t5.completed_at = Some(now - Duration::hours(2));
    let t5 = store.write().await.add(t5).await.unwrap();
    ids.insert("completed".to_string(), t5.id.clone());

    // Template task (recurring) — should be excluded from plan
    let mut t6 = Todo::default_instance();
    t6.title = "Daily standup template".to_string();
    t6.is_template = true;
    t6.recurrence_rule = Some("FREQ=DAILY;BYHOUR=9".to_string());
    let t6 = store.write().await.add(t6).await.unwrap();
    ids.insert("template".to_string(), t6.id.clone());

    // No-priority overdue task — tests default priority handling
    let mut t7 = Todo::default_instance();
    t7.title = "Triage incoming tickets".to_string();
    t7.priority = None;
    t7.due_date = Some(now - Duration::days(1));
    t7.created_at = now - Duration::days(4);
    let t7 = store.write().await.add(t7).await.unwrap();
    ids.insert("no_priority".to_string(), t7.id.clone());

    (store, temp_dir, ids)
}

// ═══════════════════════════════════════════════════════════════
// 1. SCORING ALGORITHM TESTS
// ═══════════════════════════════════════════════════════════════

/// AC2: Scoring formula: (urgency × priority_weight) + (age_days × 0.1)
///
/// Urgency tiers:
///   overdue  = 10
///   today    = 5
///   tomorrow = 3
///   future   = 1
///
/// Priority weights (inverse: P1=5, P2=4, P3=3, P4=2, P5=1, None=3)
#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_scoring_overdue_p1_ranks_highest() {
    use common::{ChannelName, ChatId};
    use tools::{todo::TodoTool, RoutingContext, Tool};

    // Overdue P1 should get: (10 × 5) + (5 × 0.1) = 50.5
    // Today P2 should get:   (5 × 4) + (3 × 0.1)  = 20.3
    // Overdue P1 must rank above today P2
    let (_store, _dir, _ids) = create_planning_test_data().await;
    let pool = storage::StoragePool::connect_lazy("postgres://localhost/klyntbot_test").unwrap();
    let repo = storage::TodoRepo::new(pool.inner().clone());
    let tool = TodoTool::new(repo, 3, 18, "UTC".to_string());

    let ctx = RoutingContext::new(ChannelName::new("cli"), ChatId::new("test"));

    let result = tool
        .execute(serde_json::json!({"action": "plan"}), &ctx)
        .await
        .unwrap();

    // Verify overdue P1 task appears first with correct score
    assert!(
        result.contains("1. Fix auth token expiry bug"),
        "Overdue P1 should be ranked #1"
    );
    assert!(
        result.contains("Score: 50.5"),
        "Overdue P1 should have score 50.5"
    );

    // Verify it outranks the today P2 task
    let overdue_pos = result.find("Fix auth token expiry bug").unwrap();
    let today_pos = result.find("Implement user settings page").unwrap();
    assert!(
        overdue_pos < today_pos,
        "Overdue P1 should appear before today P2"
    );
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_scoring_no_priority_uses_default_weight() {
    use common::{ChannelName, ChatId};
    use tools::{todo::TodoTool, RoutingContext, Tool};

    // A task with priority=None should use default weight (P3 equivalent = 3)
    // Verify no-priority overdue task scores as: (10 × 3) + (4 × 0.1) = 30.4
    let (_store, _dir, _ids) = create_planning_test_data().await;
    let pool = storage::StoragePool::connect_lazy("postgres://localhost/klyntbot_test").unwrap();
    let repo = storage::TodoRepo::new(pool.inner().clone());
    let tool = TodoTool::new(repo, 3, 18, "UTC".to_string());

    let ctx = RoutingContext::new(ChannelName::new("cli"), ChatId::new("test"));

    let result = tool
        .execute(serde_json::json!({"action": "plan"}), &ctx)
        .await
        .unwrap();

    // Verify no-priority task "Triage incoming tickets" appears with correct score
    assert!(
        result.contains("Triage incoming tickets"),
        "No-priority task should be in plan"
    );
    assert!(
        result.contains("Score: 30.4"),
        "No-priority task should have score 30.4 (using P3 default)"
    );

    // Verify it's displayed as P3
    let triage_section = result.split("Triage incoming tickets").next().unwrap();
    assert!(
        result[triage_section.len()..].contains("P3"),
        "No-priority task should display as P3 (default)"
    );
}

// ═══════════════════════════════════════════════════════════════
// 2. PLAN GENERATION TESTS
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_plan_suggests_top_3_tasks() {
    use common::{ChannelName, ChatId};
    use tools::{todo::TodoTool, RoutingContext, Tool};

    // AC2: Plan should suggest exactly 3 tasks (the default count)
    let (_store, _dir, _ids) = create_planning_test_data().await;
    let pool = storage::StoragePool::connect_lazy("postgres://localhost/klyntbot_test").unwrap();
    let repo = storage::TodoRepo::new(pool.inner().clone());
    let tool = TodoTool::new(repo, 3, 18, "UTC".to_string());

    let ctx = RoutingContext::new(ChannelName::new("cli"), ChatId::new("test"));

    let result = tool
        .execute(serde_json::json!({"action": "plan"}), &ctx)
        .await
        .unwrap();

    // Verify the plan contains expected content
    assert!(result.contains("Daily plan"), "Plan should have header");
    assert!(result.contains("(3 tasks)"), "Plan should indicate 3 tasks");

    // Verify the top-ranked tasks appear (based on scoring algorithm)
    assert!(
        result.contains("Fix auth token expiry bug"),
        "Overdue P1 should be #1"
    );
    assert!(
        result.contains("Triage incoming tickets") || result.contains("Implement user settings"),
        "Plan should include high-scoring tasks"
    );

    // Verify scoring information is included
    assert!(result.contains("Score:"), "Plan should show scores");
    assert!(
        result.contains("Overdue"),
        "Plan should show urgency context"
    );

    // Future P4 task should NOT appear in top 3
    assert!(
        !result.contains("Refactor database module"),
        "Future P4 task should not be in top 3"
    );
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_plan_excludes_completed_tasks() {
    use common::{ChannelName, ChatId};
    use tools::{todo::TodoTool, RoutingContext, Tool};

    // Completed tasks (status=Done) must not appear in suggestions
    let (_store, _dir, _ids) = create_planning_test_data().await;
    let pool = storage::StoragePool::connect_lazy("postgres://localhost/klyntbot_test").unwrap();
    let repo = storage::TodoRepo::new(pool.inner().clone());
    let tool = TodoTool::new(repo, 3, 18, "UTC".to_string());

    let ctx = RoutingContext::new(ChannelName::new("cli"), ChatId::new("test"));

    let result = tool
        .execute(serde_json::json!({"action": "plan"}), &ctx)
        .await
        .unwrap();

    // Completed task "Already done task" should NOT appear in plan
    assert!(
        !result.contains("Already done task"),
        "Completed tasks should be excluded from plan"
    );
}

#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_plan_excludes_template_tasks() {
    use common::{ChannelName, ChatId};
    use tools::{todo::TodoTool, RoutingContext, Tool};

    // Recurring templates (is_template=true) must not appear in suggestions
    let (_store, _dir, _ids) = create_planning_test_data().await;
    let pool = storage::StoragePool::connect_lazy("postgres://localhost/klyntbot_test").unwrap();
    let repo = storage::TodoRepo::new(pool.inner().clone());
    let tool = TodoTool::new(repo, 3, 18, "UTC".to_string());

    let ctx = RoutingContext::new(ChannelName::new("cli"), ChatId::new("test"));

    let result = tool
        .execute(serde_json::json!({"action": "plan"}), &ctx)
        .await
        .unwrap();

    // Template task "Daily standup template" should NOT appear in plan
    assert!(
        !result.contains("Daily standup template"),
        "Template tasks should be excluded from plan"
    );
}

// ═══════════════════════════════════════════════════════════════
// 4. RESPONSE PARSING TESTS
// ═══════════════════════════════════════════════════════════════

/// AC3: Parse "yes" and variants → PlanAction::Accept
#[tokio::test]
async fn test_response_accept_variants() {
    use tools::plan_response::{parse_plan_response, PlanAction};

    // UX §4.1: "yes", "y", "ok", "go", "looks good", "let's go", "confirm", "accept"
    let accept_inputs = [
        "yes",
        "y",
        "ok",
        "go",
        "looks good",
        "let's go",
        "confirm",
        "accept",
    ];
    for input in &accept_inputs {
        let action = parse_plan_response(input, 3).unwrap();
        assert!(
            matches!(action, PlanAction::Accept),
            "Failed for: {}",
            input
        );
    }
}

/// AC3: Parse "swap X and Y" → PlanAction::Swap { pos_a, pos_b }
#[tokio::test]
async fn test_response_swap_variants() {
    use tools::plan_response::{parse_plan_response, PlanAction};

    // UX §4.1: "swap 1 and 2", "swap 1,2", "swap 1 2", "reorder 1 and 2"
    let action = parse_plan_response("swap 1 and 2", 3).unwrap();
    assert!(matches!(action, PlanAction::Swap { pos_a: 1, pos_b: 2 }));

    let action = parse_plan_response("swap 1,3", 3).unwrap();
    assert!(matches!(action, PlanAction::Swap { pos_a: 1, pos_b: 3 }));

    let action = parse_plan_response("swap 2 3", 3).unwrap();
    assert!(matches!(action, PlanAction::Swap { pos_a: 2, pos_b: 3 }));

    let action = parse_plan_response("reorder 1 and 2", 3).unwrap();
    assert!(matches!(action, PlanAction::Swap { pos_a: 1, pos_b: 2 }));
}

/// AC3: Parse "skip N" → PlanAction::Skip { pos }
#[tokio::test]
async fn test_response_skip_variants() {
    use tools::plan_response::{parse_plan_response, PlanAction};

    // UX §4.1: "skip 1", "skip #1", "remove 1", "drop 1"
    let action = parse_plan_response("skip 2", 3).unwrap();
    assert!(matches!(action, PlanAction::Skip { pos: 2 }));

    let action = parse_plan_response("drop 1", 3).unwrap();
    assert!(matches!(action, PlanAction::Skip { pos: 1 }));

    let action = parse_plan_response("skip #3", 3).unwrap();
    assert!(matches!(action, PlanAction::Skip { pos: 3 }));

    let action = parse_plan_response("remove 2", 3).unwrap();
    assert!(matches!(action, PlanAction::Skip { pos: 2 }));
}

/// AC3: Parse "defer all" → PlanAction::DeferAll
#[tokio::test]
async fn test_response_defer_variants() {
    use tools::plan_response::{parse_plan_response, PlanAction};

    // UX §4.1: "defer", "defer all", "not today", "pass", "dismiss", "nah", "no"
    let defer_inputs = [
        "defer",
        "defer all",
        "not today",
        "pass",
        "dismiss",
        "nah",
        "no",
    ];
    for input in &defer_inputs {
        let action = parse_plan_response(input, 3).unwrap();
        assert!(
            matches!(action, PlanAction::DeferAll),
            "Failed for: {}",
            input
        );
    }
}

// ═══════════════════════════════════════════════════════════════
// 8. CLI COMMAND TESTS
// ═══════════════════════════════════════════════════════════════

/// AC5: `klyntbot todo plan` manually triggers planning
#[tokio::test]
#[ignore = "requires PostgreSQL (DATABASE_URL)"]
async fn test_cli_todo_plan_command() {
    use common::{ChannelName, ChatId};
    use tools::{todo::TodoTool, RoutingContext, Tool};

    // The plan action on TodoTool should generate and return a plan
    let (_store, _dir, _ids) = create_planning_test_data().await;
    let pool = storage::StoragePool::connect_lazy("postgres://localhost/klyntbot_test").unwrap();
    let repo = storage::TodoRepo::new(pool.inner().clone());
    let tool = TodoTool::new(repo, 3, 18, "UTC".to_string());

    let ctx = RoutingContext::new(ChannelName::new("cli"), ChatId::new("test"));

    let result = tool
        .execute(serde_json::json!({"action": "plan"}), &ctx)
        .await
        .unwrap();

    // Verify the result contains plan-related content
    assert!(
        result.contains("Good morning") || result.contains("tasks") || result.contains("focus"),
        "Plan output should contain greeting or task information"
    );
}
