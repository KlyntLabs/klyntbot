//! E2E tests for coding_todo: multi-agent concurrency, plan-mode, compaction.

use bus::domain_events::{ConcurrencyClass, TodoStatus};
use common::{ChannelName, ChatId};
use feature_coding_todo::tool::{CodingTodoParams, CodingTodoTool};
use feature_coding_todo::validation::{ValidationContext, validate_write};
use std::sync::Arc;
use storage::StoragePool;
use storage::repos::TodoRepo;
use tools_core::{RoutingContext, ToolExecute};

async fn setup() -> (TodoRepo, Arc<bus::DomainEventBus>) {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    sqlx::query(
        feature_coding_todo::migrations::coding_todo_migration()
            .sql
            .as_str(),
    )
    .execute(pool.inner())
    .await
    .unwrap();
    let bus = Arc::new(bus::DomainEventBus::new(64));
    (TodoRepo::new(pool.inner().clone()), bus)
}

fn ctx(agent_id: &str, agent_profile: &str) -> RoutingContext {
    let mut ctx = RoutingContext::new(ChannelName::from("coding"), ChatId::from("thread-e2e"));
    ctx.agent_id = agent_id.into();
    ctx.agent_profile = agent_profile.into();
    ctx.plan_mode_active = false;
    ctx.previous_anti_passivity_violation = false;
    ctx.same_turn_user_msg_emitted = false;
    ctx
}

#[tokio::test]
async fn multi_agent_concurrency_rejects_exclusive_conflict() {
    let (repo, bus) = setup().await;

    // Agent A starts an exclusive item.
    let tool_a = CodingTodoTool::new(repo.clone(), bus.clone());
    let items_json = serde_json::json!([
        {"id": "a1", "title": "Refactor core", "status": "in_progress", "concurrency": "exclusive"}
    ])
    .to_string();
    tool_a
        .execute(CodingTodoParams { items_json }, &ctx("agent_a", "code"))
        .await
        .unwrap();

    // Agent B tries to start any in_progress item.
    let tool_b = CodingTodoTool::new(repo.clone(), bus.clone());
    let items_json = serde_json::json!([
        {"id": "b1", "title": "Add feature", "status": "in_progress", "concurrency": "safe"}
    ])
    .to_string();
    let result = tool_b
        .execute(CodingTodoParams { items_json }, &ctx("agent_b", "code"))
        .await;

    assert!(
        result.is_err(),
        "expected concurrency rejection when agent_a has exclusive"
    );
}

#[tokio::test]
async fn plan_mode_allows_only_pending() {
    let (repo, bus) = setup().await;
    let mut c = ctx("root", "code");
    c.plan_mode_active = true;

    let tool = CodingTodoTool::new(repo, bus);
    let items_json = serde_json::json!([
        {"id": "p1", "title": "Plan step 1", "status": "in_progress", "concurrency": "safe"}
    ])
    .to_string();
    let result = tool.execute(CodingTodoParams { items_json }, &c).await;

    assert!(
        result.is_err(),
        "plan mode should reject non-pending status"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("plan mode") || msg.contains("pending"),
        "error should mention plan mode: {}",
        msg
    );
}

#[tokio::test]
async fn compaction_preserves_created_at() {
    let (repo, bus) = setup().await;
    let tool = CodingTodoTool::new(repo.clone(), bus);
    let c = ctx("root", "code");

    // Create item.
    let items_json = serde_json::json!([
        {"id": "c1", "title": "Stable task", "status": "pending", "concurrency": "safe"}
    ])
    .to_string();
    tool.execute(CodingTodoParams { items_json }, &c)
        .await
        .unwrap();

    let row1 = repo.get("thread-e2e", "root").await.unwrap().unwrap();
    let items1: Vec<feature_coding_todo::types::TodoItem> =
        serde_json::from_str(&row1.items_json).unwrap();
    let created1 = items1[0].created_at;

    // Simulate "compaction" — re-submit same list (noop short-circuit).
    let items_json = serde_json::json!([
        {"id": "c1", "title": "Stable task", "status": "pending", "concurrency": "safe"}
    ])
    .to_string();
    tool.execute(CodingTodoParams { items_json }, &c)
        .await
        .unwrap();

    let row2 = repo.get("thread-e2e", "root").await.unwrap().unwrap();
    let items2: Vec<feature_coding_todo::types::TodoItem> =
        serde_json::from_str(&row2.items_json).unwrap();
    let created2 = items2[0].created_at;

    assert_eq!(
        created1, created2,
        "created_at should survive compaction/noop"
    );
}
