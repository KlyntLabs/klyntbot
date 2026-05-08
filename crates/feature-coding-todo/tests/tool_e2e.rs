//! End-to-end test: instantiate the tool, call execute(), assert side effects.

use bus::domain_events::{DomainEvent, TodoEvent, TodoStatus};
use bus::DomainEventBus;
use common::{ChatId, ChannelName};
use feature_coding_todo::tool::{CodingTodoParams, CodingTodoTool};
use std::sync::Arc;
use storage::repos::TodoRepo;
use storage::StoragePool;
use tools_core::{RoutingContext, ToolExecute};

async fn setup() -> (CodingTodoTool, tokio::sync::broadcast::Receiver<DomainEvent>) {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    sqlx::query(feature_coding_todo::migrations::coding_todo_migration().sql.as_str())
        .execute(pool.inner())
        .await
        .unwrap();
    let repo = TodoRepo::new(pool.inner().clone());
    let bus = Arc::new(DomainEventBus::new(64));
    let rx = bus.subscribe();
    let tool = CodingTodoTool::new(repo, bus);
    (tool, rx)
}

fn ctx(thread_id: &str) -> RoutingContext {
    let mut ctx = RoutingContext::new(
        ChannelName::from("coding"),
        ChatId::from(thread_id),
    );
    ctx.agent_id = "root".into();
    ctx.agent_profile = "root".into();
    ctx.plan_mode_active = false;
    ctx.previous_anti_passivity_violation = false;
    ctx.same_turn_user_msg_emitted = false;
    ctx
}

#[tokio::test]
async fn execute_inserts_new_list_and_publishes_events() {
    let (tool, mut rx) = setup().await;

    let items_json = serde_json::json!([
        {"id": "a", "title": "Read schema", "status": "pending", "concurrency": "safe"},
        {"id": "b", "title": "Add migration", "status": "in_progress", "concurrency": "sequential"}
    ])
    .to_string();

    let summary = tool
        .execute(CodingTodoParams { items_json }, &ctx("t1"))
        .await
        .unwrap();
    assert!(summary.contains("2 items"));
    assert!(summary.contains("1 pending"));
    assert!(summary.contains("1 in_progress"));

    // At least 2 events should have been published (added events).
    let mut event_count = 0;
    while let Ok(evt) = rx.try_recv() {
        if matches!(evt, DomainEvent::Todo(_)) {
            event_count += 1;
        }
    }
    assert!(event_count >= 2, "expected ≥2 todo events, got {}", event_count);
}

#[tokio::test]
async fn execute_rejects_two_in_progress() {
    let (tool, _rx) = setup().await;

    let items_json = serde_json::json!([
        {"id": "a", "title": "x", "status": "in_progress", "concurrency": "safe"},
        {"id": "b", "title": "y", "status": "in_progress", "concurrency": "safe"}
    ])
    .to_string();

    let result = tool
        .execute(CodingTodoParams { items_json }, &ctx("t1"))
        .await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("multiple in_progress") || msg.contains("in_progress"),
        "error should mention in_progress conflict: {}",
        msg
    );
}

#[tokio::test]
async fn execute_publishes_status_change_event() {
    let (tool, mut rx) = setup().await;
    let ctx = ctx("t1");

    // First call: create pending item.
    let items_json = serde_json::json!([
        {"id": "a", "title": "Read schema", "status": "pending", "concurrency": "safe"}
    ])
    .to_string();
    tool.execute(CodingTodoParams { items_json }, &ctx)
        .await
        .unwrap();

    // Drain initial events.
    while rx.try_recv().is_ok() {}

    // Second call: move to in_progress.
    let items_json = serde_json::json!([
        {"id": "a", "title": "Read schema", "status": "in_progress", "concurrency": "safe"}
    ])
    .to_string();
    tool.execute(CodingTodoParams { items_json }, &ctx)
        .await
        .unwrap();

    let mut found_status_change = false;
    while let Ok(evt) = rx.try_recv() {
        if let DomainEvent::Todo(TodoEvent::StateChanged { from, to, .. }) = evt {
            if from == TodoStatus::Pending && to == TodoStatus::InProgress {
                found_status_change = true;
            }
        }
    }
    assert!(found_status_change, "expected StateChanged event Pending->InProgress");
}

#[tokio::test]
async fn execute_publishes_cancelled_event() {
    let (tool, mut rx) = setup().await;
    let ctx = ctx("t1");

    // First call: create item.
    let items_json = serde_json::json!([
        {"id": "a", "title": "Read schema", "status": "pending", "concurrency": "safe"}
    ])
    .to_string();
    tool.execute(CodingTodoParams { items_json }, &ctx)
        .await
        .unwrap();

    // Drain initial events.
    while rx.try_recv().is_ok() {}

    // Second call: remove item (empty list).
    let items_json = "[]".to_string();
    tool.execute(CodingTodoParams { items_json }, &ctx)
        .await
        .unwrap();

    let mut found_cancelled = false;
    while let Ok(evt) = rx.try_recv() {
        if let DomainEvent::Todo(TodoEvent::Cancelled { item_id, .. }) = evt {
            if item_id == "a" {
                found_cancelled = true;
            }
        }
    }
    assert!(found_cancelled, "expected Cancelled event for item 'a'");
}
