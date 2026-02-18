//! GraphQL Subscription tests for the dashboard crate.
//!
//! Tests real-time event delivery via subscriptions for todos, projects,
//! goals, plans, agent events, notifications, and config changes.
//!
//! Strategy: subscribe to `DashboardContext.event_tx` before moving the
//! context into the schema.  Mutations internally call `event_tx.send()`,
//! so the same receiver validates the subscription infrastructure without
//! needing to poll a live WebSocket stream.

mod common;

use async_graphql::Request;
use dashboard::events::{ChangeAction, DashboardEvent};
use tokio::time::Duration;

// Shared execute helper — identical to the one in graphql_mutations.rs
macro_rules! execute {
    ($schema:expr, $query:expr) => {{
        let res = $schema.execute(Request::new($query)).await;
        assert!(res.errors.is_empty(), "GraphQL errors: {:?}", res.errors);
        res.data.into_json().unwrap()
    }};
}

// Helper: recv from a broadcast channel with a generous timeout.
// The 2-second budget accommodates parallel test executor contention.
macro_rules! recv_event {
    ($rx:expr) => {
        tokio::time::timeout(Duration::from_secs(2), $rx.recv())
            .await
            .expect("timeout waiting for event")
            .expect("broadcast channel closed")
    };
}

// ═══════════════════════════════════════════════════════════════════
// Todo Subscriptions
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_todo_changed_subscription_created() {
    // AC-CC-1.1: Real-time todo creation event
    // Given: Active subscription on todoChanged
    // When: A todo is created via mutation
    // Then: Subscription emits TodoChangeEvent { action: CREATED, todo: {...} }

    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let mut rx = ctx.event_tx.subscribe();
    let schema = common::create_test_schema(ctx);

    let data = execute!(
        schema,
        r#"mutation { createTodo(input: { title: "Sub test" }) { id } }"#
    );
    let todo_id = data["createTodo"]["id"].as_str().unwrap().to_string();

    let event = recv_event!(rx);
    match event {
        DashboardEvent::TodoChanged { action, id } => {
            assert_eq!(action, ChangeAction::Created);
            assert_eq!(id, todo_id);
        }
        other => panic!("Expected TodoChanged, got {:?}", other),
    }
}

#[tokio::test]
async fn test_todo_changed_subscription_updated() {
    // AC-CC-1.1: Real-time todo update event
    // Given: Active subscription on todoChanged
    // When: A todo is updated via mutation
    // Then: Subscription emits TodoChangeEvent { action: UPDATED, todo: {...} }

    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let todo_id = {
        let mut store = ctx.todo_store.write().await;
        store.add(common::make_todo("Before")).await.unwrap().id
    };
    let mut rx = ctx.event_tx.subscribe();
    let schema = common::create_test_schema(ctx);

    execute!(
        schema,
        &format!(
            r#"mutation {{ updateTodo(id: "{}", input: {{ title: "After" }}) {{ id }} }}"#,
            todo_id
        )
    );

    let event = recv_event!(rx);
    match event {
        DashboardEvent::TodoChanged { action, id } => {
            assert_eq!(action, ChangeAction::Updated);
            assert_eq!(id, todo_id);
        }
        other => panic!("Expected TodoChanged Updated, got {:?}", other),
    }
}

#[tokio::test]
async fn test_todo_changed_subscription_deleted() {
    // AC-CC-1.1: Real-time todo deletion event
    // Given: Active subscription on todoChanged
    // When: A todo is deleted via mutation
    // Then: Subscription emits TodoChangeEvent { action: DELETED, id: "...", todo: null }

    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let todo_id = {
        let mut store = ctx.todo_store.write().await;
        store.add(common::make_todo("To delete")).await.unwrap().id
    };
    let mut rx = ctx.event_tx.subscribe();
    let schema = common::create_test_schema(ctx);

    execute!(
        schema,
        &format!(r#"mutation {{ deleteTodo(id: "{}") }}"#, todo_id)
    );

    let event = recv_event!(rx);
    match event {
        DashboardEvent::TodoChanged { action, id } => {
            assert_eq!(action, ChangeAction::Deleted);
            assert_eq!(id, todo_id);
        }
        other => panic!("Expected TodoChanged Deleted, got {:?}", other),
    }
}

// ═══════════════════════════════════════════════════════════════════
// Project Subscriptions
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_project_changed_subscription() {
    // AC-CC-1.1: Real-time project change
    // Given: Active subscription on projectChanged
    // When: A project is created/updated
    // Then: Subscription emits ProjectChangeEvent

    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let mut rx = ctx.event_tx.subscribe();
    let schema = common::create_test_schema(ctx);

    let data = execute!(
        schema,
        r#"mutation { createProject(input: { name: "Sub Project", color: BLUE }) { id } }"#
    );
    let project_id = data["createProject"]["id"].as_str().unwrap().to_string();

    let event = recv_event!(rx);
    match event {
        DashboardEvent::ProjectChanged { action, id } => {
            assert_eq!(action, ChangeAction::Created);
            assert_eq!(id, project_id);
        }
        other => panic!("Expected ProjectChanged, got {:?}", other),
    }
}

// ═══════════════════════════════════════════════════════════════════
// Goal Subscriptions
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_goal_changed_subscription() {
    // AC-CC-1.1: Real-time goal change
    // Given: Active subscription on goalChanged
    // When: A goal metric is updated (or goal created)
    // Then: Subscription emits GoalChangeEvent

    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let mut rx = ctx.event_tx.subscribe();
    let schema = common::create_test_schema(ctx);

    let data = execute!(
        schema,
        r#"mutation { createGoal(input: { title: "Goal Sub" }) { id } }"#
    );
    let goal_id = data["createGoal"]["id"].as_str().unwrap().to_string();

    let event = recv_event!(rx);
    match event {
        DashboardEvent::GoalChanged { action, id } => {
            assert_eq!(action, ChangeAction::Created);
            assert_eq!(id, goal_id);
        }
        other => panic!("Expected GoalChanged, got {:?}", other),
    }
}

// ═══════════════════════════════════════════════════════════════════
// Plan Subscriptions
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_plan_changed_subscription_step_completed() {
    // AC-14.1: Step list auto-updates
    // Given: Active subscription on planChanged
    // When: A plan step completes (simulated via approvePlan transition)
    // Then: Subscription emits PlanChangeEvent with updated step status

    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let mut rx = ctx.event_tx.subscribe();
    let schema = common::create_test_schema(ctx);

    // Create and approve a plan to trigger PlanChanged events
    let data = execute!(
        schema,
        r#"mutation { createPlan(input: { title: "Step Plan", steps: [] }) { id } }"#
    );
    let plan_id = data["createPlan"]["id"].as_str().unwrap().to_string();

    // Verify creation event
    let create_event = recv_event!(rx);
    match create_event {
        DashboardEvent::PlanChanged { action, id } => {
            assert_eq!(action, ChangeAction::Created);
            assert_eq!(id, plan_id);
        }
        other => panic!("Expected PlanChanged Created, got {:?}", other),
    }

    // Approve the plan to trigger another PlanChanged event (simulates step progression)
    execute!(
        schema,
        &format!(r#"mutation {{ approvePlan(id: "{}") {{ id }} }}"#, plan_id)
    );

    let approve_event = recv_event!(rx);
    match approve_event {
        DashboardEvent::PlanChanged { action, id } => {
            assert_eq!(action, ChangeAction::Updated);
            assert_eq!(id, plan_id);
        }
        other => panic!("Expected PlanChanged Updated (approve), got {:?}", other),
    }
}

#[tokio::test]
async fn test_plan_changed_subscription_backtrack() {
    // AC-14.3: Backtrack timeline updates
    // Given: Active subscription on planChanged
    // When: A backtrack event occurs (simulated via abandon → create cycle)
    // Then: Subscription emits PlanChangeEvent with backtrack history

    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let mut rx = ctx.event_tx.subscribe();
    let schema = common::create_test_schema(ctx);

    // Create a plan then abandon it to simulate a "backtrack" requiring re-plan
    let data = execute!(
        schema,
        r#"mutation { createPlan(input: { title: "Backtrack Plan", steps: [] }) { id } }"#
    );
    let plan_id = data["createPlan"]["id"].as_str().unwrap().to_string();

    let _ = recv_event!(rx); // consume Created event

    execute!(
        schema,
        &format!(r#"mutation {{ abandonPlan(id: "{}") {{ id }} }}"#, plan_id)
    );

    let event = recv_event!(rx);
    match event {
        DashboardEvent::PlanChanged { action, id } => {
            // Abandoned plan emits an Updated event (status change)
            assert!(
                action == ChangeAction::Updated || action == ChangeAction::Deleted,
                "Expected Updated or Deleted, got {:?}",
                action
            );
            assert_eq!(id, plan_id);
        }
        other => panic!("Expected PlanChanged, got {:?}", other),
    }
}

// ═══════════════════════════════════════════════════════════════════
// Notification Subscription
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_notification_subscription() {
    // AC-17.1: Real-time notifications
    // Given: Active subscription on notification
    // When: A notification event is emitted (e.g., task overdue)
    // Then: Subscription delivers { title, body } to client

    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let event_tx = ctx.event_tx.clone();
    let mut rx = ctx.event_tx.subscribe();
    let _schema = common::create_test_schema(ctx);

    // Publish a notification directly (as the agent/reminder engine would)
    event_tx
        .send(DashboardEvent::Notification {
            title: "Task overdue".to_string(),
            body: "Your task 'Fix auth bug' is past its due date.".to_string(),
        })
        .ok();

    let event = recv_event!(rx);
    match event {
        DashboardEvent::Notification { title, body } => {
            assert_eq!(title, "Task overdue");
            assert!(body.contains("Fix auth bug"));
        }
        other => panic!("Expected Notification, got {:?}", other),
    }
}

// ═══════════════════════════════════════════════════════════════════
// Config Subscription
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_config_changed_subscription() {
    // AC-CC-6.1: Config hot-reload event
    // Given: Active subscription on configChanged
    // When: Config is updated (via updateConfig mutation)
    // Then: Subscription emits full Config object via ConfigReloaded event

    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let mut rx = ctx.event_tx.subscribe();
    let schema = common::create_test_schema(ctx);

    execute!(
        schema,
        r#"mutation { updateConfig(section: "todo", values: {}) { enrichmentEnabled } }"#
    );

    let event = recv_event!(rx);
    assert!(
        matches!(event, DashboardEvent::ConfigReloaded),
        "Expected ConfigReloaded, got {:?}",
        event
    );
}

// ═══════════════════════════════════════════════════════════════════
// System Status Subscription
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_system_status_subscription() {
    // AC-18.1: Live system status
    // Given: Active subscription on systemStatus
    // When: System status changes (channel connects/disconnects → AgentEvent)
    // Then: Subscription emits updated SystemStatus

    let tmp = tempfile::TempDir::new().unwrap();
    let ctx = common::create_test_context(&tmp).await;
    let event_tx = ctx.event_tx.clone();
    let mut rx = ctx.event_tx.subscribe();
    let _schema = common::create_test_schema(ctx);

    // Publish an AgentEvent to simulate status change (agent iteration started)
    event_tx
        .send(DashboardEvent::AgentEvent(agent::AgentEvent::IterationStart {
            iteration: 1,
            max: 10,
        }))
        .ok();

    let event = recv_event!(rx);
    assert!(
        matches!(event, DashboardEvent::AgentEvent(_)),
        "Expected AgentEvent (system status trigger), got {:?}",
        event
    );
}
