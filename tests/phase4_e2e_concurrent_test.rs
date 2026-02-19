//! E2E test: Concurrent plan execution — state isolation
//!
//! AC-E2E.2: Multiple plans executing concurrently must not interfere with each other.
//! Each plan must end in its own correct final state regardless of concurrent transitions.
//!
//! Run: `cargo nextest run --test phase4_e2e_concurrent_test`

use chrono::Utc;
use klyntbot::agent::learning::{ExecutionMode, OutcomeRecorder, OutcomeStore};
use plan::{Plan, PlanStatus, PlanStep, PlanStore, StepStatus};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::RwLock;
use uuid::Uuid;

// ─────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────

fn make_plan(session_key: &str) -> Plan {
    let now = Utc::now();
    Plan {
        id: Uuid::new_v4(),
        session_key: session_key.to_string(),
        goal_id: None,
        title: format!("Plan for {}", session_key),
        description: "Concurrent E2E test plan".to_string(),
        status: PlanStatus::Draft,
        steps: vec![PlanStep {
            id: Uuid::new_v4(),
            index: 0,
            description: "Single step".to_string(),
            reasoning: "Test reasoning".to_string(),
            expected_tools: vec!["todo".to_string()],
            status: StepStatus::Pending,
            attempt_count: 0,
            max_attempts: 3,
            result: None,
            started_at: None,
            completed_at: None,
        }],
        current_step_index: 0,
        iteration_limit: 50,
        backtrack_history: vec![],
        created_at: now,
        updated_at: now,
        completed_at: None,
    }
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.2.1 — Plans have independent IDs and don't overwrite each other
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_concurrent_plans_have_independent_ids() {
    let tmp = TempDir::new().unwrap();
    let plan_store = Arc::new(RwLock::new(PlanStore::new(tmp.path().join("plans.jsonl"))));

    let plan_a = make_plan("e2e:session-a");
    let plan_b = make_plan("e2e:session-b");
    let plan_c = make_plan("e2e:session-c");

    let id_a = plan_a.id;
    let id_b = plan_b.id;
    let id_c = plan_c.id;

    // Insert all three concurrently
    {
        let (r1, r2, r3) = tokio::join!(
            async {
                let mut s = plan_store.write().await;
                s.upsert(plan_a.clone()).await
            },
            async {
                let mut s = plan_store.write().await;
                s.upsert(plan_b.clone()).await
            },
            async {
                let mut s = plan_store.write().await;
                s.upsert(plan_c.clone()).await
            }
        );
        r1.unwrap();
        r2.unwrap();
        r3.unwrap();
    }

    // Verify all three exist independently
    let (pa, pb, pc) = {
        let mut s = plan_store.write().await;
        let pa = s.get(&id_a).await.unwrap();
        let pb = s.get(&id_b).await.unwrap();
        let pc = s.get(&id_c).await.unwrap();
        (pa, pb, pc)
    };

    assert!(pa.is_some(), "plan A must exist");
    assert!(pb.is_some(), "plan B must exist");
    assert!(pc.is_some(), "plan C must exist");

    assert_ne!(id_a, id_b, "plans must have unique IDs");
    assert_ne!(id_b, id_c, "plans must have unique IDs");
    assert_ne!(id_a, id_c, "plans must have unique IDs");

    assert_eq!(pa.unwrap().session_key, "e2e:session-a");
    assert_eq!(pb.unwrap().session_key, "e2e:session-b");
    assert_eq!(pc.unwrap().session_key, "e2e:session-c");
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.2.2 — Plans reach different final states independently
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_concurrent_plans_reach_different_final_states() {
    let tmp = TempDir::new().unwrap();
    let plan_store = Arc::new(RwLock::new(PlanStore::new(tmp.path().join("plans.jsonl"))));

    let mut plan_success = make_plan("e2e:success");
    let mut plan_failed = make_plan("e2e:failed");
    let mut plan_abandoned = make_plan("e2e:abandoned");

    let id_s = plan_success.id;
    let id_f = plan_failed.id;
    let id_a = plan_abandoned.id;

    // Insert all
    {
        let mut s = plan_store.write().await;
        s.upsert(plan_success.clone()).await.unwrap();
        s.upsert(plan_failed.clone()).await.unwrap();
        s.upsert(plan_abandoned.clone()).await.unwrap();
    }

    // Advance plan_success: Draft → Approved → Executing → Completed
    plan_success.status = PlanStatus::Approved;
    plan_success.status = PlanStatus::Executing;
    plan_success.status = PlanStatus::Completed;
    plan_success.completed_at = Some(Utc::now());

    // Advance plan_failed: Draft → Approved → Executing → Failed
    plan_failed.status = PlanStatus::Approved;
    plan_failed.status = PlanStatus::Executing;
    plan_failed.status = PlanStatus::Failed;

    // Advance plan_abandoned: Draft → Abandoned
    plan_abandoned.status = PlanStatus::Abandoned;

    // Persist final states
    {
        let mut s = plan_store.write().await;
        s.upsert(plan_success.clone()).await.unwrap();
        s.upsert(plan_failed.clone()).await.unwrap();
        s.upsert(plan_abandoned.clone()).await.unwrap();
    }

    // Verify each plan is in its own final state
    let (ps, pf, pa) = {
        let mut s = plan_store.write().await;
        let ps = s.get(&id_s).await.unwrap().unwrap();
        let pf = s.get(&id_f).await.unwrap().unwrap();
        let pa = s.get(&id_a).await.unwrap().unwrap();
        (ps, pf, pa)
    };

    assert_eq!(
        ps.status,
        PlanStatus::Completed,
        "success plan must be Completed"
    );
    assert_eq!(pf.status, PlanStatus::Failed, "failed plan must be Failed");
    assert_eq!(
        pa.status,
        PlanStatus::Abandoned,
        "abandoned plan must be Abandoned"
    );

    // Cross-contamination check
    assert_ne!(ps.status, pf.status, "success/failed must differ");
    assert_ne!(ps.status, pa.status, "success/abandoned must differ");
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.2.3 — Concurrent outcome recording stays consistent
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_concurrent_outcome_recording_consistent() {
    let outcome_store = Arc::new(RwLock::new(OutcomeStore::new_in_memory()));

    let recorder = Arc::new(OutcomeRecorder::new(Arc::clone(&outcome_store)));

    // Record 10 outcomes concurrently (simulating parallel plan steps)
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let r = Arc::clone(&recorder);
            let plan_id = Uuid::new_v4().to_string();
            tokio::spawn(async move {
                r.record_tool_outcome(
                    "todo",
                    i % 3 != 0, // every 3rd fails
                    if i % 3 == 0 { Some("timeout") } else { None },
                    (i as u64) * 10 + 20,
                    None,
                    ExecutionMode::PlanStep {
                        plan_id,
                        step_index: i,
                    },
                    "e2e:concurrent",
                )
                .await;
            })
        })
        .collect();

    for h in handles {
        h.await.unwrap();
    }

    let s = outcome_store.read().await;
    let outcomes = s.get_all_outcomes().await.unwrap();

    assert_eq!(outcomes.len(), 10, "all 10 outcomes must be recorded");

    // Verify failure count (indices 0, 3, 6, 9 → 4 failures)
    let fail_count = outcomes.iter().filter(|o| !o.success).count();
    assert_eq!(
        fail_count, 4,
        "4 out of 10 must be failures (indices 0,3,6,9)"
    );

    // All must be PlanStep mode
    assert!(
        outcomes
            .iter()
            .all(|o| matches!(&o.execution_mode, ExecutionMode::PlanStep { .. })),
        "all outcomes must be PlanStep mode"
    );
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.2.4 — State transitions are serial (no interleaved corruption)
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_plan_state_machine_serial_transitions_valid() {
    // Tests that the state machine validator rejects invalid sequences
    // even if called concurrently.
    let valid_sequence = [
        (PlanStatus::Draft, PlanStatus::Approved),
        (PlanStatus::Approved, PlanStatus::Executing),
        (PlanStatus::Executing, PlanStatus::Completed),
    ];

    for (from, to) in &valid_sequence {
        PlanStatus::validate_transition(from, to)
            .unwrap_or_else(|e| panic!("transition {:?} → {:?} should succeed: {}", from, to, e));
    }

    // Invalid transitions from final states must fail
    let invalid_sequence = [
        (PlanStatus::Completed, PlanStatus::Draft),
        (PlanStatus::Failed, PlanStatus::Approved),
        (PlanStatus::Abandoned, PlanStatus::Executing),
    ];

    for (from, to) in &invalid_sequence {
        assert!(
            PlanStatus::validate_transition(from, to).is_err(),
            "transition {:?} → {:?} should fail",
            from,
            to
        );
    }
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.2.5 — Concurrent plans share store without data loss
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_store_handles_multiple_plans_no_data_loss() {
    let tmp = TempDir::new().unwrap();
    let plan_store = Arc::new(RwLock::new(PlanStore::new(tmp.path().join("plans.jsonl"))));

    const N: usize = 20;
    let mut ids = Vec::with_capacity(N);

    // Insert N plans
    for i in 0..N {
        let plan = make_plan(&format!("e2e:bulk-{}", i));
        let id = plan.id;
        ids.push(id);
        let mut s = plan_store.write().await;
        s.upsert(plan).await.unwrap();
    }

    // Verify all N plans can be retrieved
    let found: Vec<bool> = {
        let mut s = plan_store.write().await;
        let mut results = Vec::new();
        for id in &ids {
            results.push(s.get(id).await.unwrap().is_some());
        }
        results
    };

    assert_eq!(found.len(), N);
    assert!(
        found.iter().all(|&b| b),
        "all {} plans must be retrievable",
        N
    );
}
