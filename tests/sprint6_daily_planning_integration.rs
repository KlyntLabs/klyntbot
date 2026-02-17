//! Sprint 6 Integration Tests — Daily Planning Skill
//!
//! Tests the daily planning feature end-to-end: scoring algorithm, plan generation,
//! response parsing, cron integration, CLI commands, and config toggle.
//!
//! ## Acceptance Criteria Mapping
//!
//! | AC# | Criterion                                          | Test(s)                                     |
//! |-----|----------------------------------------------------|--------------------------------------------|
//! | AC1 | Cron job runs daily at digest time                  | test_cron_triggers_daily_plan               |
//! | AC2 | Notification shows suggested focus order + reasoning| test_plan_notification_format               |
//! | AC3 | User can reply "yes", "swap X,Y", "skip N", "defer"| test_response_* (4 tests)                   |
//! | AC4 | Agent auto-focuses tasks on confirmation            | test_accept_plan_focuses_tasks              |
//! | AC5 | `klyntbot todo plan` manually triggers planning     | test_cli_todo_plan_command                  |
//! | AC6 | Config `todo.daily_planning: false` disables feature| test_config_disabled_skips_planning          |
//! | AC7 | Zero clippy warnings                               | (CI check, not a runtime test)              |
//!
//! ## Test Data Strategy
//!
//! Tests use `create_planning_test_data()` which produces a mix of:
//! - Overdue P1 task (should rank highest)
//! - Due-today P2 task (should rank second)
//! - Due-tomorrow P3 task (should rank third)
//! - Future P4 task (should NOT appear in top 3)
//! - Completed task (should be excluded)
//! - Template task (should be excluded)

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
async fn test_scoring_overdue_p1_ranks_highest() {
    use common::{ChannelName, ChatId};
    use tools::{todo::TodoTool, RoutingContext, Tool};

    // Overdue P1 should get: (10 × 5) + (5 × 0.1) = 50.5
    // Today P2 should get:   (5 × 4) + (3 × 0.1)  = 20.3
    // Overdue P1 must rank above today P2
    let (store, _dir, _ids) = create_planning_test_data().await;
    let tool = TodoTool::new(store, 3, 18, "UTC".to_string());

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
async fn test_scoring_today_ranks_above_tomorrow() {
    // Today P2:    (5 × 4) + (3 × 0.1) = 20.3
    // Tomorrow P3: (3 × 3) + (1 × 0.1) = 9.1
    //
    // TODO: Implement once DailyPlanner exists
    // let (store, _dir, ids) = create_planning_test_data().await;
    // let planner = DailyPlanner::new(store, config);
    // let plan = planner.generate().await.unwrap();
    // assert_eq!(plan.suggestions[1].task_id, ids["today_p2"]);
}

#[tokio::test]
async fn test_scoring_tiebreak_by_created_at() {
    // When two tasks have identical scores, the older one (earlier created_at)
    // should rank higher — deterministic tiebreak.
    //
    // TODO: Implement
    // Create two tasks with same priority, same urgency, different created_at
    // Verify older task ranks first
}

#[tokio::test]
async fn test_scoring_no_priority_uses_default_weight() {
    use common::{ChannelName, ChatId};
    use tools::{todo::TodoTool, RoutingContext, Tool};

    // A task with priority=None should use default weight (P3 equivalent = 3)
    // Verify no-priority overdue task scores as: (10 × 3) + (4 × 0.1) = 30.4
    let (store, _dir, _ids) = create_planning_test_data().await;
    let tool = TodoTool::new(store, 3, 18, "UTC".to_string());

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

#[tokio::test]
async fn test_scoring_age_factor_rewards_older_tasks() {
    // Age factor: age_days × 0.1
    // A 10-day-old task gets +1.0, a 1-day-old task gets +0.1
    // This prevents old tasks from being perpetually deferred
    //
    // TODO: Implement
}

// ═══════════════════════════════════════════════════════════════
// 2. PLAN GENERATION TESTS
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_plan_suggests_top_3_tasks() {
    use common::{ChannelName, ChatId};
    use tools::{todo::TodoTool, RoutingContext, Tool};

    // AC2: Plan should suggest exactly 3 tasks (the default count)
    let (store, _dir, _ids) = create_planning_test_data().await;
    let tool = TodoTool::new(store, 3, 18, "UTC".to_string());

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
async fn test_plan_excludes_completed_tasks() {
    use common::{ChannelName, ChatId};
    use tools::{todo::TodoTool, RoutingContext, Tool};

    // Completed tasks (status=Done) must not appear in suggestions
    let (store, _dir, _ids) = create_planning_test_data().await;
    let tool = TodoTool::new(store, 3, 18, "UTC".to_string());

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
async fn test_plan_excludes_template_tasks() {
    use common::{ChannelName, ChatId};
    use tools::{todo::TodoTool, RoutingContext, Tool};

    // Recurring templates (is_template=true) must not appear in suggestions
    let (store, _dir, _ids) = create_planning_test_data().await;
    let tool = TodoTool::new(store, 3, 18, "UTC".to_string());

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

#[tokio::test]
async fn test_plan_with_fewer_than_3_eligible_tasks() {
    // AC2 partial plan: When fewer than 3 tasks are eligible,
    // return as many as available (1 or 2)
    //
    // TODO: Implement
    // Create store with only 1 eligible task
    // let plan = planner.generate().await.unwrap();
    // assert_eq!(plan.suggestions.len(), 1);
}

#[tokio::test]
async fn test_plan_empty_when_no_eligible_tasks() {
    // Edge case: No tasks at all, or all completed/templates
    // Should return empty suggestions with "all clear" greeting
    //
    // TODO: Implement
    // let plan = planner.generate().await.unwrap();
    // assert!(plan.suggestions.is_empty());
}

#[tokio::test]
async fn test_plan_includes_reasoning_for_each_suggestion() {
    // AC2: Each suggestion must include a human-readable reasoning string
    // e.g., "Overdue by 2 days", "Due today", "Due tomorrow"
    //
    // TODO: Implement
    // for suggestion in &plan.suggestions {
    //     assert!(!suggestion.reasoning.is_empty());
    // }
}

#[tokio::test]
async fn test_plan_includes_due_context() {
    // UX spec §3.1: Each suggestion has due_context like "Overdue by 2 days"
    //
    // TODO: Implement
    // assert!(plan.suggestions[0].due_context.as_ref().unwrap().contains("Overdue"));
    // assert!(plan.suggestions[1].due_context.as_ref().unwrap().contains("Due today"));
}

// ═══════════════════════════════════════════════════════════════
// 3. NOTIFICATION FORMAT TESTS
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_plan_notification_format() {
    // AC2: Notification includes greeting, calendar events (if configured),
    // and numbered task list with priority + reasoning
    //
    // TODO: Implement
    // let output = format_plan(&plan, FormatTarget::Chat);
    // assert!(output.contains("Good morning"));
    // assert!(output.contains("1."));
    // assert!(output.contains("P1"));
    // assert!(output.contains("yes"));
    // assert!(output.contains("swap"));
}

#[tokio::test]
async fn test_plan_notification_omits_calendar_when_not_configured() {
    // UX §6.3: No calendar section when calendar sync is not configured
    //
    // TODO: Implement
    // assert!(!output.contains("Calendar"));
}

#[tokio::test]
async fn test_plan_empty_state_notification() {
    // UX §3.3: "All clear" message when no tasks need attention
    //
    // TODO: Implement
    // assert!(output.contains("All clear"));
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

#[tokio::test]
async fn test_response_case_insensitive() {
    // UX §4.2: Input is trimmed and lowercased
    //
    // TODO: Implement
    // assert!(matches!(parse_plan_response("YES", 3).unwrap(), PlanAction::Accept));
    // assert!(matches!(parse_plan_response("  Ok  ", 3).unwrap(), PlanAction::Accept));
    // assert!(matches!(parse_plan_response("SWAP 1 AND 2", 3).unwrap(), PlanAction::Swap { .. }));
}

#[tokio::test]
async fn test_response_strips_punctuation() {
    // UX §4.2: Leading/trailing punctuation stripped ("yes!", "ok.")
    //
    // TODO: Implement
    // assert!(matches!(parse_plan_response("yes!", 3).unwrap(), PlanAction::Accept));
    // assert!(matches!(parse_plan_response("ok.", 3).unwrap(), PlanAction::Accept));
}

// ─── Validation Error Cases ───────────────────────────────────

#[tokio::test]
async fn test_response_skip_out_of_range() {
    // UX §4.3: "skip 5" on 3-task plan → validation error
    //
    // TODO: Implement
    // let result = parse_plan_response("skip 5", 3);
    // assert!(result.is_err());
    // assert!(result.unwrap_err().contains("only 3 tasks"));
}

#[tokio::test]
async fn test_response_swap_same_position() {
    // UX §4.3: "swap 1 and 1" → validation error
    //
    // TODO: Implement
    // let result = parse_plan_response("swap 1 and 1", 3);
    // assert!(result.is_err());
    // assert!(result.unwrap_err().contains("same position"));
}

#[tokio::test]
async fn test_response_unrecognized_input() {
    // UX §4.3: Random text → error with help options
    //
    // TODO: Implement
    // let result = parse_plan_response("hello", 3);
    // assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════
// 5. PLAN EXECUTION TESTS
// ═══════════════════════════════════════════════════════════════

/// AC4: Accepting a plan focuses all suggested tasks
#[tokio::test]
async fn test_accept_plan_focuses_tasks() {
    // After "yes" response, all plan tasks should have:
    //   status = Doing
    //   focused_at = Some(now)
    //
    // TODO: Implement
    // let (store, _dir, ids) = create_planning_test_data().await;
    // let plan = planner.generate().await.unwrap();
    // planner.execute_action(PlanAction::Accept, &plan).await.unwrap();
    //
    // let s = store.read().await;
    // for suggestion in &plan.suggestions {
    //     let task = s.get(&suggestion.task_id).await.unwrap().unwrap();
    //     assert_eq!(task.status, TodoStatus::Doing);
    //     assert!(task.focused_at.is_some());
    // }
}

#[tokio::test]
async fn test_swap_reorders_plan() {
    // "swap 1 and 3" should exchange positions in the plan
    //
    // TODO: Implement
    // let plan = planner.generate().await.unwrap();
    // let original_first = plan.suggestions[0].task_id.clone();
    // let original_third = plan.suggestions[2].task_id.clone();
    //
    // let updated = planner.apply_swap(&plan, 1, 3).unwrap();
    // assert_eq!(updated.suggestions[0].task_id, original_third);
    // assert_eq!(updated.suggestions[2].task_id, original_first);
}

#[tokio::test]
async fn test_skip_removes_and_promotes() {
    // "skip 2" removes task #2 and promotes the next eligible task
    //
    // TODO: Implement
    // let plan = planner.generate().await.unwrap();
    // let skipped_id = plan.suggestions[1].task_id.clone();
    //
    // let updated = planner.apply_skip(&plan, 2, &store).await.unwrap();
    // assert_eq!(updated.suggestions.len(), 3);
    // assert!(!updated.suggestions.iter().any(|s| s.task_id == skipped_id));
}

#[tokio::test]
async fn test_defer_all_takes_no_action() {
    // "defer all" should dismiss the plan without focusing any tasks
    //
    // TODO: Implement
    // planner.execute_action(PlanAction::DeferAll, &plan).await.unwrap();
    // for suggestion in &plan.suggestions {
    //     let task = store.read().await.get(&suggestion.task_id).await.unwrap().unwrap();
    //     assert_eq!(task.status, TodoStatus::Todo);
    //     assert!(task.focused_at.is_none());
    // }
}

// ─── Edge Cases ───────────────────────────────────────────────

#[tokio::test]
async fn test_accept_with_stale_task() {
    // UX §6.2: A task completed between plan send and "yes" response
    // should be skipped, with next task promoted
    //
    // TODO: Implement
    // Generate plan, then mark task #1 as Done, then accept
    // Verify only 2 tasks focused, confirmation mentions "1 was already completed"
}

#[tokio::test]
async fn test_accept_replaces_existing_focus() {
    // UX §6.1: When focus slots are full, accepting replaces current focus
    //
    // TODO: Implement
    // Pre-focus 3 tasks, generate plan, accept → old tasks unfocused, new ones focused
}

// ═══════════════════════════════════════════════════════════════
// 6. STATE MACHINE TESTS
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_state_machine_no_plan_to_pending() {
    // UX §5: Trigger moves state from NO_PLAN → PENDING
    //
    // TODO: Implement
    // let state = PlanState::NoPlan;
    // let plan = planner.generate().await.unwrap();
    // assert!(matches!(state.transition(plan), PlanState::Pending(_)));
}

#[tokio::test]
async fn test_state_machine_pending_accept_to_accepted() {
    // UX §5: Accept moves PENDING → ACCEPTED
    //
    // TODO: Implement
}

#[tokio::test]
async fn test_state_machine_pending_defer_to_deferred() {
    // UX §5: Defer moves PENDING → DEFERRED
    //
    // TODO: Implement
}

#[tokio::test]
async fn test_state_machine_pending_swap_stays_pending() {
    // UX §5: Swap/skip stays PENDING (updated plan)
    //
    // TODO: Implement
}

#[tokio::test]
async fn test_plan_already_confirmed_today() {
    // UX §6.4: Re-triggering after acceptance shows "already confirmed" message
    //
    // TODO: Implement
}

#[tokio::test]
async fn test_plan_expires_at_midnight() {
    // UX §5.1: Plans expire at end of day (midnight local time)
    //
    // TODO: Implement
}

// ═══════════════════════════════════════════════════════════════
// 7. CRON INTEGRATION TESTS
// ═══════════════════════════════════════════════════════════════

/// AC1: Cron job triggers daily planning at digest time
#[tokio::test]
async fn test_cron_triggers_daily_plan() {
    // The daily planning should register a cron job at the configured
    // digest time (default: "08:00")
    //
    // TODO: Implement
    // Verify cron job name is "__klyntbot_daily_plan" or similar
    // Verify it calls the planning engine
}

#[tokio::test]
async fn test_cron_respects_config_time() {
    // The cron trigger time comes from config.todo.notifications.daily_digest_time
    //
    // TODO: Implement
    // config.todo.notifications.daily_digest_time = "09:30"
    // Verify cron schedule matches "30 9 * * *"
}

// ═══════════════════════════════════════════════════════════════
// 8. CLI COMMAND TESTS
// ═══════════════════════════════════════════════════════════════

/// AC5: `klyntbot todo plan` manually triggers planning
#[tokio::test]
async fn test_cli_todo_plan_command() {
    use common::{ChannelName, ChatId};
    use tools::{todo::TodoTool, RoutingContext, Tool};

    // The plan action on TodoTool should generate and return a plan
    let (store, _dir, _ids) = create_planning_test_data().await;
    let tool = TodoTool::new(store, 3, 18, "UTC".to_string());

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

#[tokio::test]
async fn test_cli_todo_plan_accept_flag() {
    // `klyntbot todo plan --accept` auto-accepts the plan
    //
    // TODO: Implement
    // let result = tool.execute(json!({"action": "plan", "accept": true}), &ctx).await.unwrap();
    // Verify tasks are focused
}

#[tokio::test]
async fn test_cli_todo_plan_skip_flag() {
    // `klyntbot todo plan --skip 2` skips task #2 before accepting
    //
    // TODO: Implement
}

// ═══════════════════════════════════════════════════════════════
// 9. CONFIG TOGGLE TESTS
// ═══════════════════════════════════════════════════════════════

/// AC6: Config `todo.daily_planning: false` disables feature
#[tokio::test]
async fn test_config_disabled_skips_planning() {
    // When daily_planning is false, plan action should return a message
    // indicating the feature is disabled
    //
    // TODO: Implement
    // config.todo.daily_planning = false;
    // let result = planner.generate().await.unwrap();
    // assert!(result.is_none() or error message about disabled)
}

#[tokio::test]
async fn test_config_daily_planning_defaults_to_true() {
    // DailyPlanningConfig::default().enabled should be true
    //
    // TODO: Implement
    // let config = DailyPlanningConfig::default();
    // assert!(config.enabled);
}

#[tokio::test]
async fn test_config_serde_roundtrip() {
    // DailyPlanningConfig serializes to camelCase and deserializes back
    //
    // TODO: Implement
    // let config = DailyPlanningConfig { enabled: false };
    // let json = serde_json::to_string(&config).unwrap();
    // assert!(json.contains("dailyPlanning") || json.contains("enabled"));
    // let loaded: DailyPlanningConfig = serde_json::from_str(&json).unwrap();
    // assert_eq!(loaded.enabled, false);
}

// ═══════════════════════════════════════════════════════════════
// 10. CALENDAR INTEGRATION TESTS
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_plan_includes_calendar_events() {
    // UX §3.1: When calendar is configured, plan includes today's events
    //
    // TODO: Implement with mock CalendarHandler
    // plan.calendar_events should contain today's events
}

#[tokio::test]
async fn test_plan_calendar_events_sorted_by_time() {
    // Calendar events should be sorted chronologically
    //
    // TODO: Implement
}
