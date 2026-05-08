//! CodingTodoTool — the LLM-facing tool registration.

use crate::diff::{compute_diff, diff_to_events, EventMeta};
use crate::errors::CodingTodoError;
use crate::id::assign_missing_ids;
use crate::plan_mode;
use crate::types::{TodoItem, TodoItemInput};
use crate::validation::{validate_write, ValidationContext};
use async_trait::async_trait;
use bus::domain_events::{ConcurrencyClass, TodoStatus};
use bus::DomainEventBus;
use common::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use storage::repos::TodoRepo;
use tools_core::{RoutingContext, ToolExecute};
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};

#[derive(ToolDerive, Clone)]
#[tool(
    name = "coding_todo",
    description = "Maintain a per-agent todo list for the current coding session. Pass full list to overwrite; pass empty list to clear. Items: {id?, title, status, concurrency, blocked_by?, blocked_reason?, delegated_to?}. status enum: pending|in_progress|done|blocked. concurrency enum: safe|sequential|exclusive. See KLYNTBOT-coding.md for usage rules.",
    params = "CodingTodoParams",
    allowed_channels = "coding_only",
    concurrency_safe = "false",
    approval_class = "safe"
)]
pub struct CodingTodoTool {
    repo: TodoRepo,
    bus: Arc<DomainEventBus>,
}

#[derive(ToolParamsDerive, Deserialize, Debug, Clone)]
pub struct CodingTodoParams {
    /// JSON array of objects matching `TodoItemInput` shape.
    pub items_json: String,
}

impl CodingTodoTool {
    pub fn new(repo: TodoRepo, bus: Arc<DomainEventBus>) -> Self {
        Self { repo, bus }
    }
}

#[async_trait]
impl ToolExecute for CodingTodoTool {
    type Params = CodingTodoParams;

    async fn execute(&self, params: Self::Params, ctx: &RoutingContext) -> Result<String> {
        let thread_id = ctx.chat_id.as_str().to_string();
        let agent_id = ctx.agent_id.clone();
        let agent_profile = ctx.agent_profile.clone();

        let inputs: Vec<TodoItemInput> = serde_json::from_str(&params.items_json)
            .map_err(CodingTodoError::InvalidItemShape)?;

        let inputs = assign_missing_ids(inputs);

        // Fetch all rows for the thread in one query; partition into current vs others.
        let all_rows = self.repo.list_for_thread(&thread_id).await?;
        let (prior_row, other_rows): (Option<_>, Vec<_>) = {
            let (mine, others): (Vec<_>, Vec<_>) = all_rows
                .into_iter()
                .partition(|r| r.agent_id == agent_id);
            (mine.into_iter().next(), others)
        };

        let (prior_items, prior_created_ats): (Vec<TodoItemInput>, HashMap<String, jiff::Timestamp>) =
            match &prior_row {
                Some(row) => {
                    let parsed: Vec<TodoItem> =
                        serde_json::from_str(&row.items_json).map_err(|e| {
                            CodingTodoError::CorruptedDbData {
                                reason: format!(
                                    "failed to parse items_json for agent {}: {}",
                                    row.agent_id, e
                                ),
                            }
                        })?;
                    let created_ats: HashMap<String, jiff::Timestamp> = parsed
                        .iter()
                        .map(|i| (i.id.clone(), i.created_at))
                        .collect();
                    let inputs: Vec<TodoItemInput> =
                        parsed.into_iter().map(TodoItemInput::from).collect();
                    (inputs, created_ats)
                }
                None => (Vec::new(), HashMap::new()),
            };

        let plan_mode_active = ctx.plan_mode_active;
        let plan_session_id_opt = ctx.plan_session_id.clone();

        let other_in_progress: Vec<(String, String, ConcurrencyClass)> = other_rows
            .into_iter()
            .flat_map(|row| {
                let parsed = serde_json::from_str::<Vec<TodoItem>>(&row.items_json);
                match parsed {
                    Ok(items) => items.into_iter().filter_map(move |i| {
                        if matches!(i.status, TodoStatus::InProgress) {
                            Some((row.agent_id.clone(), i.id, i.concurrency))
                        } else {
                            None
                        }
                    }).collect::<Vec<_>>(),
                    Err(_) => Vec::new(), // skip corrupted rows for cross-agent checks
                }
            })
            .collect();

        let val_ctx = ValidationContext {
            agent_id: &agent_id,
            agent_profile: &agent_profile,
            plan_mode_active,
            previous_anti_passivity_violation: ctx.previous_anti_passivity_violation,
            same_turn_user_msg_emitted: ctx.same_turn_user_msg_emitted,
            other_agents_in_progress: &other_in_progress,
        };

        let validated = validate_write(inputs, &val_ctx)?;
        let diff = compute_diff(&prior_items, &validated);

        // If nothing changed and we already have a row, skip the write.
        if diff.is_empty() && prior_row.is_some() {
            let counts = count_statuses(&validated);
            return Ok(build_summary(
                &thread_id,
                &agent_id,
                validated.len(),
                counts.pending,
                counts.in_progress,
                counts.done,
                counts.blocked,
                &diff,
            ));
        }

        let now = jiff::Timestamp::now();
        let materialized: Vec<TodoItem> = validated
            .iter()
            .map(|i| materialize_item(i, &prior_created_ats, now))
            .collect::<std::result::Result<Vec<_>, CodingTodoError>>()?;

        let items_json = serde_json::to_string(&materialized).map_err(|e| {
            CodingTodoError::Storage(common::KlyntbotError::Storage(format!(
                "failed to serialize items: {}",
                e
            )))
        })?;

        self.repo
            .upsert(
                &thread_id,
                &agent_id,
                &items_json,
                plan_session_id_opt.as_deref(),
            )
            .await?;

        let meta = EventMeta {
            thread_id: thread_id.clone(),
            agent_id: agent_id.clone(),
            agent_profile: agent_profile.clone(),
            timestamp: now,
        };
        let events = diff_to_events(&diff, &validated, &meta);
        for evt in events {
            self.bus.publish_todo(evt);
        }
        if let Some(plan_session_id) = &plan_session_id_opt {
            if !diff.is_empty() || prior_row.is_none() {
                self.bus.publish_todo(plan_mode::plan_proposed_event(
                    &thread_id,
                    plan_session_id,
                    &validated,
                    now,
                ));
            }
        }

        let counts = count_statuses(&materialized);
        Ok(build_summary(
            &thread_id,
            &agent_id,
            materialized.len(),
            counts.pending,
            counts.in_progress,
            counts.done,
            counts.blocked,
            &diff,
        ))
    }
}

fn materialize_item(
    input: &TodoItemInput,
    prior_created_ats: &HashMap<String, jiff::Timestamp>,
    now: jiff::Timestamp,
) -> std::result::Result<TodoItem, CodingTodoError> {
    let id = input.id.clone().ok_or_else(|| CodingTodoError::MissingId {
        title: input.title.clone(),
    })?;
    let created_at = prior_created_ats.get(&id).copied().unwrap_or(now);
    Ok(TodoItem {
        id,
        title: input.title.clone(),
        status: input.status,
        concurrency: input.concurrency,
        blocked_reason: input.blocked_reason.clone(),
        blocked_by: input.blocked_by.clone(),
        delegated_to: input.delegated_to.clone(),
        created_at,
        updated_at: now,
    })
}

#[derive(Default)]
struct StatusCounts {
    pending: usize,
    in_progress: usize,
    done: usize,
    blocked: usize,
}

fn count_statuses<T: StatusAccessor>(items: &[T]) -> StatusCounts {
    let mut counts = StatusCounts::default();
    for item in items {
        match item.status() {
            TodoStatus::Pending => counts.pending += 1,
            TodoStatus::InProgress => counts.in_progress += 1,
            TodoStatus::Done => counts.done += 1,
            TodoStatus::Blocked => counts.blocked += 1,
        }
    }
    counts
}

trait StatusAccessor {
    fn status(&self) -> TodoStatus;
}

impl StatusAccessor for TodoItem {
    fn status(&self) -> TodoStatus { self.status }
}

impl StatusAccessor for TodoItemInput {
    fn status(&self) -> TodoStatus { self.status }
}

fn build_summary(
    thread_id: &str,
    agent_id: &str,
    total: usize,
    pending: usize,
    in_progress: usize,
    done: usize,
    blocked: usize,
    diff: &crate::diff::DiffSummary,
) -> String {
    format!(
        "Updated coding_todo for agent {} in thread {}.\n  {} items: {} pending, {} in_progress, {} done, {} blocked\n  Diff vs prior: +{} added, +{} status_changed, +{} cancelled",
        agent_id, thread_id, total, pending, in_progress, done, blocked,
        diff.added.len(), diff.status_changed.len(), diff.cancelled.len(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    fn ctx() -> RoutingContext {
        let mut ctx = RoutingContext::new(
            common::ChannelName::from("coding"),
            common::ChatId::from("test-thread"),
        );
        ctx.agent_id = "test-agent".to_string();
        ctx.agent_profile = "code".to_string();
        ctx.plan_mode_active = false;
        ctx.previous_anti_passivity_violation = false;
        ctx.same_turn_user_msg_emitted = false;
        ctx
    }

    async fn setup() -> CodingTodoTool {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        sqlx::query(crate::migrations::coding_todo_migration().sql.as_str())
            .execute(pool.inner())
            .await
            .unwrap();
        let bus = Arc::new(DomainEventBus::new(16));
        CodingTodoTool::new(TodoRepo::new(pool.inner().clone()), bus)
    }

    #[tokio::test]
    async fn test_create_single_item() {
        let tool = setup().await;
        let items_json = serde_json::json!([
            {"title": "Implement feature X", "status": "pending", "concurrency": "safe"}
        ])
        .to_string();

        let result = tool
            .execute(CodingTodoParams { items_json }, &ctx())
            .await
            .unwrap();

        assert!(result.contains("test-agent"));
        assert!(result.contains("1 items: 1 pending"));
    }

    #[tokio::test]
    async fn test_update_existing_item() {
        let tool = setup().await;
        let ctx = ctx();

        let items_json = serde_json::json!([
            {"title": "Task 1", "status": "pending", "concurrency": "safe"}
        ])
        .to_string();
        tool.execute(CodingTodoParams { items_json }, &ctx)
            .await
            .unwrap();

        let items_json = serde_json::json!([
            {"id": "t1", "title": "Task 1", "status": "done", "concurrency": "safe"}
        ])
        .to_string();
        let result = tool
            .execute(CodingTodoParams { items_json }, &ctx)
            .await
            .unwrap();

        assert!(result.contains("0 pending, 0 in_progress, 1 done"));
    }

    #[tokio::test]
    async fn test_clear_items() {
        let tool = setup().await;
        let ctx = ctx();

        let items_json = serde_json::json!([
            {"title": "Task 1", "status": "pending", "concurrency": "safe"}
        ])
        .to_string();
        tool.execute(CodingTodoParams { items_json }, &ctx)
            .await
            .unwrap();

        let result = tool
            .execute(
                CodingTodoParams {
                    items_json: "[]".to_string(),
                },
                &ctx,
            )
            .await
            .unwrap();

        assert!(result.contains("0 items"));
    }

    #[tokio::test]
    async fn test_invalid_json_rejected() {
        let tool = setup().await;
        let result = tool
            .execute(
                CodingTodoParams {
                    items_json: "not json".to_string(),
                },
                &ctx(),
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_missing_required_field_rejected() {
        let tool = setup().await;
        let items_json = serde_json::json!([{"status": "pending", "concurrency": "safe"}]).to_string();
        let result = tool
            .execute(CodingTodoParams { items_json }, &ctx())
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_noop_update_skips_write() {
        let tool = setup().await;
        let ctx = ctx();

        let items_json = serde_json::json!([
            {"id": "t1", "title": "Task 1", "status": "pending", "concurrency": "safe"}
        ])
        .to_string();
        tool.execute(CodingTodoParams { items_json: items_json.clone() }, &ctx)
            .await
            .unwrap();

        let row_before = tool.repo.get("test-thread", "test-agent").await.unwrap().unwrap();

        // Submit identical list — should short-circuit without rewriting the row.
        tool.execute(CodingTodoParams { items_json }, &ctx)
            .await
            .unwrap();

        let row_after = tool.repo.get("test-thread", "test-agent").await.unwrap().unwrap();
        assert_eq!(row_before.updated_at, row_after.updated_at);
    }

    #[tokio::test]
    async fn test_created_at_preserved_on_update() {
        let tool = setup().await;
        let ctx = ctx();

        let items_json = serde_json::json!([
            {"id": "t1", "title": "Task 1", "status": "pending", "concurrency": "safe"}
        ])
        .to_string();
        tool.execute(CodingTodoParams { items_json }, &ctx)
            .await
            .unwrap();

        let row = tool.repo.get("test-thread", "test-agent").await.unwrap().unwrap();
        let first_items: Vec<TodoItem> = serde_json::from_str(&row.items_json).unwrap();
        let first_created = first_items[0].created_at;

        // Re-submit with status change — id must match for preservation.
        let items_json = serde_json::json!([
            {"id": "t1", "title": "Task 1", "status": "in_progress", "concurrency": "safe"}
        ])
        .to_string();
        tool.execute(CodingTodoParams { items_json }, &ctx)
            .await
            .unwrap();

        let row = tool.repo.get("test-thread", "test-agent").await.unwrap().unwrap();
        let second_items: Vec<TodoItem> = serde_json::from_str(&row.items_json).unwrap();
        let second_created = second_items[0].created_at;

        assert_eq!(first_created, second_created);
    }
}
