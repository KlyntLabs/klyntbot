//! Phase 4 Plan Refactor Integration Tests — P7: Zero-copy &mut Plan
//!
//! Tests the zero-copy refactor of PlanStore to use `get_mut()` and
//! `persist_latest()` instead of `upsert(plan.clone())`.
//!
//! ## Acceptance Criteria Mapping
//!
//! | AC#     | Criterion                                           | Test(s)                                    |
//! |---------|-----------------------------------------------------|--------------------------------------------|
//! | AC-P7.1 | get_mut(id) returns &mut Plan for in-place mutation | test_get_mut_returns_mutable_reference      |
//! | AC-P7.2 | persist_latest(id) writes without caller clone      | test_persist_latest_writes_current_state    |
//! | AC-P7.3 | get_mut + persist_latest roundtrip matches disk     | test_get_mut_persist_latest_roundtrip       |
//! | AC-P7.4 | Multiple mutations via get_mut accumulate correctly | test_multiple_mutations_accumulate          |

use chrono::Utc;
use plan::{Plan, PlanStatus, PlanStep, PlanStore, StepStatus};
use tempfile::tempdir;
use uuid::Uuid;

// ─── Test Fixtures ────────────────────────────────────────────────────────────

fn test_plan(title: &str, session_key: &str) -> Plan {
    let now = Utc::now();
    Plan {
        id: Uuid::new_v4(),
        session_key: session_key.to_string(),
        goal_id: None,
        title: title.to_string(),
        description: format!("Test plan: {}", title),
        status: PlanStatus::Draft,
        steps: vec![],
        current_step_index: 0,
        iteration_limit: 50,
        backtrack_history: vec![],
        created_at: now,
        updated_at: now,
        completed_at: None,
    }
}

fn test_step(description: &str, index: usize) -> PlanStep {
    PlanStep {
        id: Uuid::new_v4(),
        index,
        description: description.to_string(),
        reasoning: "Test step".to_string(),
        expected_tools: vec![],
        status: StepStatus::Pending,
        attempt_count: 0,
        max_attempts: 3,
        result: None,
        started_at: None,
        completed_at: None,
    }
}

// ─── AC-P7.1: get_mut returns mutable reference ───────────────────────────────

/// AC-P7.1: PlanStore::get_mut() returns Option<&mut Plan> for in-place mutation.
///
/// Given: a plan inserted into the store
/// When: get_mut(id) is called
/// Then: a mutable reference is returned, and mutating it changes the in-memory state
#[tokio::test]
async fn test_get_mut_returns_mutable_reference() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("plans.jsonl");
    let mut store = PlanStore::new(path);

    let plan = test_plan("Original Title", "session-1");
    let id = plan.id;
    store.upsert(plan).await.unwrap();

    // get_mut should return Some(&mut Plan)
    {
        let plan_mut = store.get_mut(&id).await.unwrap();
        assert!(
            plan_mut.is_some(),
            "get_mut should return Some for existing plan"
        );

        // Mutate in-place
        let p = plan_mut.unwrap();
        p.title = "Mutated Title".to_string();
        p.current_step_index = 3;
    }

    // In-memory state should reflect the mutation
    let retrieved = store.get(&id).await.unwrap();
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.title, "Mutated Title");
    assert_eq!(retrieved.current_step_index, 3);
}

/// AC-P7.1b: get_mut on non-existent id returns None
#[tokio::test]
async fn test_get_mut_missing_plan_returns_none() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("plans.jsonl");
    let mut store = PlanStore::new(path);

    let missing_id = Uuid::new_v4();
    let result = store.get_mut(&missing_id).await.unwrap();
    assert!(result.is_none(), "get_mut on missing id should return None");
}

// ─── AC-P7.2: persist_latest writes current in-memory state ──────────────────

/// AC-P7.2: persist_latest(id) writes the current in-memory plan to the journal.
///
/// Given: a plan was mutated via get_mut (in-memory only)
/// When: persist_latest(id) is called
/// Then: a new store reloading from disk sees the mutation
#[tokio::test]
async fn test_persist_latest_writes_current_state() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("plans.jsonl");
    let mut store = PlanStore::new(path.clone());

    let plan = test_plan("Initial", "session-1");
    let id = plan.id;
    store.upsert(plan).await.unwrap();

    // Mutate in-place via get_mut
    {
        let p = store.get_mut(&id).await.unwrap().unwrap();
        p.title = "Persisted Mutation".to_string();
        p.status = PlanStatus::Approved;
    }

    // Persist without passing a Plan clone
    store.persist_latest(&id).await.unwrap();

    // A fresh store reloading from disk should see the mutation
    let mut fresh = PlanStore::new(path);
    let loaded = fresh.get(&id).await.unwrap();
    assert!(loaded.is_some());
    let loaded = loaded.unwrap();
    assert_eq!(loaded.title, "Persisted Mutation");
    assert_eq!(loaded.status, PlanStatus::Approved);
}

/// AC-P7.2b: persist_latest on non-existent id is a no-op (doesn't error)
#[tokio::test]
async fn test_persist_latest_missing_plan_is_noop() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("plans.jsonl");
    let mut store = PlanStore::new(path);

    let missing_id = Uuid::new_v4();
    // Should not error
    store.persist_latest(&missing_id).await.unwrap();
}

// ─── AC-P7.3: Full roundtrip: get_mut → mutate → persist_latest → reload ─────

/// AC-P7.3: get_mut + persist_latest roundtrip: mutations survive a store reload.
///
/// Given: a plan with steps inserted into the store
/// When: steps are mutated via get_mut and persisted via persist_latest
/// Then: a fresh store reloads the exact mutated step state
#[tokio::test]
async fn test_get_mut_persist_latest_roundtrip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("plans.jsonl");
    let mut store = PlanStore::new(path.clone());

    let mut plan = test_plan("Step Plan", "session-1");
    plan.steps.push(test_step("First step", 0));
    plan.steps.push(test_step("Second step", 1));
    let id = plan.id;
    store.upsert(plan).await.unwrap();

    // Simulate step execution via get_mut
    {
        let p = store.get_mut(&id).await.unwrap().unwrap();
        p.steps[0].status = StepStatus::Executing;
        p.steps[0].attempt_count = 1;
        p.current_step_index = 0;
        p.status = PlanStatus::Executing;
    }
    store.persist_latest(&id).await.unwrap();

    // Mark step 0 completed, advance to step 1
    {
        let p = store.get_mut(&id).await.unwrap().unwrap();
        p.steps[0].status = StepStatus::Completed;
        p.steps[0].result = Some("Step 0 output".to_string());
        p.current_step_index = 1;
    }
    store.persist_latest(&id).await.unwrap();

    // Reload and verify
    let mut fresh = PlanStore::new(path);
    let loaded = fresh.get(&id).await.unwrap().unwrap();

    assert_eq!(loaded.status, PlanStatus::Executing);
    assert_eq!(loaded.current_step_index, 1);
    assert_eq!(loaded.steps[0].status, StepStatus::Completed);
    assert_eq!(loaded.steps[0].result.as_deref(), Some("Step 0 output"));
    assert_eq!(loaded.steps[0].attempt_count, 1);
    assert_eq!(loaded.steps[1].status, StepStatus::Pending);
}

// ─── AC-P7.4: Multiple mutations accumulate correctly ─────────────────────────

/// AC-P7.4: Multiple sequential get_mut mutations accumulate — latest persist_latest wins.
///
/// Given: a plan in the store
/// When: get_mut is called twice with different mutations, each followed by persist_latest
/// Then: the second mutation is reflected in the reloaded plan (last-write-wins)
#[tokio::test]
async fn test_multiple_mutations_accumulate() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("plans.jsonl");
    let mut store = PlanStore::new(path.clone());

    let plan = test_plan("Accumulate", "session-1");
    let id = plan.id;
    store.upsert(plan).await.unwrap();

    // First mutation + persist
    {
        let p = store.get_mut(&id).await.unwrap().unwrap();
        p.title = "First Mutation".to_string();
        p.current_step_index = 1;
    }
    store.persist_latest(&id).await.unwrap();

    // Second mutation + persist
    {
        let p = store.get_mut(&id).await.unwrap().unwrap();
        p.title = "Second Mutation".to_string();
        p.current_step_index = 2;
    }
    store.persist_latest(&id).await.unwrap();

    // Reload: should see second mutation
    let mut fresh = PlanStore::new(path);
    let loaded = fresh.get(&id).await.unwrap().unwrap();
    assert_eq!(loaded.title, "Second Mutation");
    assert_eq!(loaded.current_step_index, 2);
}

/// AC-P7.4b: persist_latest after get_mut increases journal_len
#[tokio::test]
async fn test_persist_latest_increments_journal_len() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("plans.jsonl");
    let mut store = PlanStore::new(path);

    let plan = test_plan("Journal Test", "session-1");
    let id = plan.id;
    store.upsert(plan).await.unwrap();

    let len_before = store.journal_len;

    {
        let p = store.get_mut(&id).await.unwrap().unwrap();
        p.title = "Updated".to_string();
    }
    store.persist_latest(&id).await.unwrap();

    assert_eq!(
        store.journal_len,
        len_before + 1,
        "persist_latest should add one journal entry"
    );
}
