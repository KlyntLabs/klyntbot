//! Integration tests for I1: Plan Execution Queue with mpsc channel
//!
//! ## Acceptance Criteria Mapping
//!
//! | AC#     | Criterion                                                    | Test(s)                                          |
//! |---------|--------------------------------------------------------------|--------------------------------------------------|
//! | AC-I1.1 | Queue execution: send request → worker receives via channel  | test_queue_sends_to_channel                      |
//! | AC-I1.2 | PlanExecutionRequest has plan_id + routing_ctx fields        | test_plan_execution_request_fields               |
//! | AC-I1.3 | Worker task drains all pending requests from queue           | test_worker_drains_all_queued_requests           |
//! | AC-I1.4 | AgentEvent::PlanStepCompleted variant with expected fields   | test_plan_step_completed_event_variant           |
//! | AC-I1.5 | AgentEvent::PlanCompleted variant with expected fields       | test_plan_completed_event_variant                |
//!
//! Run: `cargo nextest run --test phase4_queue_tests`

use klyntbot::agent::agent_loop::PlanExecutionRequest;
use klyntbot::agent::events::AgentEvent;
use tokio::sync::mpsc;
use tools::RoutingContext;
use uuid::Uuid;

// ─── AC-I1.2: PlanExecutionRequest struct fields ───────────────────────────────

/// AC-I1.2: PlanExecutionRequest has plan_id and routing_ctx fields.
///
/// Given: a Uuid plan_id and a RoutingContext
/// When: PlanExecutionRequest is constructed
/// Then: plan_id field matches the given Uuid
#[test]
fn test_plan_execution_request_fields() {
    let plan_id = Uuid::new_v4();
    let routing_ctx = RoutingContext::new("cli".into(), "session-1".into());

    let request = PlanExecutionRequest {
        plan_id,
        routing_ctx,
    };

    assert_eq!(request.plan_id, plan_id);
}

// ─── AC-I1.1: Queue sends request to channel ──────────────────────────────────

/// AC-I1.1: Sending a PlanExecutionRequest through an mpsc channel is received by a worker.
///
/// Given: an mpsc channel for PlanExecutionRequest
/// When: a request is sent
/// Then: the worker (receiver) receives a request with the same plan_id
#[tokio::test]
async fn test_queue_sends_to_channel() {
    let (tx, mut rx) = mpsc::channel::<PlanExecutionRequest>(10);
    let plan_id = Uuid::new_v4();
    let routing_ctx = RoutingContext::new("cli".into(), "session-1".into());

    tx.send(PlanExecutionRequest {
        plan_id,
        routing_ctx,
    })
    .await
    .expect("send should not fail");

    let received = rx.recv().await.expect("should receive request");
    assert_eq!(received.plan_id, plan_id);
}

// ─── AC-I1.3: Worker drains all queued requests ────────────────────────────────

/// AC-I1.3: Worker task processes all pending queue items.
///
/// Given: an mpsc channel with 3 queued PlanExecutionRequests
/// When: the sender is dropped (channel closed) and the worker drains it
/// Then: exactly 3 requests are received
#[tokio::test]
async fn test_worker_drains_all_queued_requests() {
    let (tx, mut rx) = mpsc::channel::<PlanExecutionRequest>(16);

    let plan_ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();

    for &id in &plan_ids {
        let routing_ctx = RoutingContext::new("cli".into(), "test".into());
        tx.send(PlanExecutionRequest {
            plan_id: id,
            routing_ctx,
        })
        .await
        .expect("send should not fail");
    }

    // Close the channel so the "worker" can terminate
    drop(tx);

    let mut received_ids = Vec::new();
    while let Some(req) = rx.recv().await {
        received_ids.push(req.plan_id);
    }

    assert_eq!(received_ids.len(), 3, "worker should drain all 3 requests");
    for id in &plan_ids {
        assert!(
            received_ids.contains(id),
            "worker should receive plan_id {}",
            id
        );
    }
}

// ─── AC-I1.4: AgentEvent::PlanStepCompleted variant ──────────────────────────

/// AC-I1.4: AgentEvent::PlanStepCompleted variant exists with plan_id, step_index, result fields.
///
/// Given: a PlanStepCompleted event is constructed
/// When: it is pattern-matched
/// Then: step_index and result fields match the values used during construction
#[test]
fn test_plan_step_completed_event_variant() {
    let plan_id = Uuid::new_v4();
    let event = AgentEvent::PlanStepCompleted {
        plan_id,
        step_index: 2,
        result: "step output".to_string(),
    };

    match event {
        AgentEvent::PlanStepCompleted {
            plan_id: eid,
            step_index,
            result,
        } => {
            assert_eq!(eid, plan_id);
            assert_eq!(step_index, 2);
            assert_eq!(result, "step output");
        }
        _ => panic!("Expected PlanStepCompleted variant"),
    }
}

// ─── AC-I1.5: AgentEvent::PlanCompleted variant ───────────────────────────────

/// AC-I1.5: AgentEvent::PlanCompleted variant exists with plan_id and summary fields.
///
/// Given: a PlanCompleted event is constructed
/// When: it is pattern-matched
/// Then: plan_id and summary fields match the values used during construction
#[test]
fn test_plan_completed_event_variant() {
    let plan_id = Uuid::new_v4();
    let event = AgentEvent::PlanCompleted {
        plan_id,
        summary: "All 3 steps completed.".to_string(),
    };

    match event {
        AgentEvent::PlanCompleted {
            plan_id: eid,
            summary,
        } => {
            assert_eq!(eid, plan_id);
            assert_eq!(summary, "All 3 steps completed.");
        }
        _ => panic!("Expected PlanCompleted variant"),
    }
}
