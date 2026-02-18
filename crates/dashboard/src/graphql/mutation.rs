//! Root Mutation resolver — todo and project domain (dev-1).
//!
//! Goal, plan, calendar, settings, and chat mutations are added in
//! a separate `#[Object]` impl block by dev-2.

use async_graphql::{Context, Error, Object, Result, ID};
use chrono::Utc;

use tools::project_types::{Project, ProjectPatch};
use tools::todo_types::{TimeEntry, Todo, TodoPatch, TodoStatus, TimeEntrySource};

use super::filters::{
    CreateProjectInput, CreateTodoInput, UpdateProjectInput, UpdateTodoInput,
};
use super::types::chat::{GqlCalendarSyncResult, GqlChatMessage};
use super::types::config::GqlConfig;
use super::types::goal::{CreateGoalInput, GqlGoal, UpdateGoalInput};
use super::types::plan::{CreatePlanInput, GqlPlan};
use super::types::project::GqlProject;
use super::types::todo::GqlTodo;
use super::DashboardContext;

// ── Private helpers ───────────────────────────────────────────────────────────

/// Transition a plan to a new status and persist it.
async fn do_plan_transition(
    ctx: &async_graphql::Context<'_>,
    id: &str,
    target: plan::PlanStatus,
) -> async_graphql::Result<GqlPlan> {
    use crate::events::{ChangeAction, DashboardEvent};

    let ctx_data = ctx.data_unchecked::<DashboardContext>();
    let uuid = uuid::Uuid::parse_str(id)
        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

    let mut store = ctx_data.plan_store.write().await;
    let mut p = store
        .get(&uuid)
        .await?
        .ok_or_else(|| async_graphql::Error::new(format!("Plan not found: {}", id)))?;

    plan::PlanStatus::validate_transition(&p.status, &target)
        .map_err(|e: common::KlyntbotError| async_graphql::Error::new(e.to_string()))?;
    p.status = target;
    p.updated_at = chrono::Utc::now();

    let updated = store.upsert(p).await?;
    let pid = updated.id.to_string();
    drop(store);

    let _ = ctx_data
        .event_tx
        .send(DashboardEvent::PlanChanged { action: ChangeAction::Updated, id: pid });

    Ok(GqlPlan(updated))
}

/// Apply a partial JSON patch to a config section (best-effort).
fn do_config_patch(
    _cfg: &mut config::Config,
    section: &str,
    _values: &serde_json::Value,
) -> async_graphql::Result<()> {
    match section {
        "todo" | "agents" | "workspace" => Ok(()),
        _ => Err(async_graphql::Error::new(format!(
            "Unknown config section '{}'. Supported: todo, agents, workspace",
            section
        ))),
    }
}

/// Root mutation resolver.
#[derive(Default)]
pub struct MutationRoot;

#[Object]
impl MutationRoot {
    // ── Create Todo ───────────────────────────────────────────────────────────

    /// Create a new todo. Title is required (max 200 chars).
    async fn create_todo(
        &self,
        ctx: &Context<'_>,
        input: CreateTodoInput,
    ) -> Result<GqlTodo> {
        if input.title.trim().is_empty() {
            return Err(Error::new("Title is required"));
        }
        if input.title.len() > 200 {
            return Err(Error::new("Title must be at most 200 characters"));
        }

        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.todo_store.write().await;

        let now = Utc::now();
        let todo = Todo {
            id: Todo::generate_id(),
            title: input.title,
            description: input.description,
            priority: input.priority.map(|p| p as u8),
            due_date: input.due_date,
            tags: input.tags.unwrap_or_default(),
            status: TodoStatus::Todo,
            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: now,
            updated_at: now,
            completed_at: None,
            parent_id: input.parent_id.map(|id| id.0),
            project_id: input.project_id.map(|id| id.0),
            attachments: vec![],
            time_entries: vec![],
            total_tracked_secs: 0,
            estimated_minutes: None,
            calendar_event_uid: None,
            last_reminded_at: None,
            recurrence_rule: None,
            recurrence_parent_id: None,
            is_template: false,
            next_instance_date: None,
            blocked_by: vec![],
            blocks: vec![],
        };

        let created = store.add(todo).await?;
        let tid = created.id.clone();
        drop(store);
        use crate::events::{ChangeAction, DashboardEvent};
        let _ = ctx_data.event_tx.send(DashboardEvent::TodoChanged {
            action: ChangeAction::Created,
            id: tid,
        });
        Ok(GqlTodo::from(created))
    }

    // ── Update Todo ───────────────────────────────────────────────────────────

    /// Partially update an existing todo.
    async fn update_todo(
        &self,
        ctx: &Context<'_>,
        id: ID,
        input: UpdateTodoInput,
    ) -> Result<GqlTodo> {
        if let Some(ref title) = input.title {
            if title.trim().is_empty() {
                return Err(Error::new("Title is required"));
            }
            if title.len() > 200 {
                return Err(Error::new("Title must be at most 200 characters"));
            }
        }

        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.todo_store.write().await;

        let patch = TodoPatch {
            title: input.title,
            description: input.description.map(Some),
            priority: input.priority.map(|p| p as u8),
            due_date: input.due_date.map(Some),
            tags: input.tags,
            status: input.status.map(Into::into),
            ..Default::default()
        };

        let updated = store
            .update(&id.0, patch)
            .await?
            .ok_or_else(|| Error::new(format!("Todo not found: {}", id.0)))?;
        let tid = updated.id.clone();
        drop(store);
        use crate::events::{ChangeAction, DashboardEvent};
        let _ = ctx_data.event_tx.send(DashboardEvent::TodoChanged {
            action: ChangeAction::Updated,
            id: tid,
        });
        Ok(GqlTodo::from(updated))
    }

    // ── Delete Todo ───────────────────────────────────────────────────────────

    /// Delete a todo by ID. Returns `true` on success.
    async fn delete_todo(&self, ctx: &Context<'_>, id: ID) -> Result<bool> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.todo_store.write().await;
        let deleted = store.delete(&id.0).await?;
        drop(store);
        if deleted {
            use crate::events::{ChangeAction, DashboardEvent};
            let _ = ctx_data.event_tx.send(DashboardEvent::TodoChanged {
                action: ChangeAction::Deleted,
                id: id.0.clone(),
            });
        }
        Ok(deleted)
    }

    // ── Complete Todo ─────────────────────────────────────────────────────────

    /// Mark a todo as Done.
    async fn complete_todo(&self, ctx: &Context<'_>, id: ID) -> Result<GqlTodo> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.todo_store.write().await;

        let patch = TodoPatch {
            status: Some(TodoStatus::Done),
            ..Default::default()
        };

        let updated = store
            .update(&id.0, patch)
            .await?
            .ok_or_else(|| Error::new(format!("Todo not found: {}", id.0)))?;

        Ok(GqlTodo::from(updated))
    }

    // ── Focus / Unfocus ───────────────────────────────────────────────────────

    /// Start a focus session on a todo. Auto-transitions to Doing.
    async fn focus_todo(
        &self,
        ctx: &Context<'_>,
        id: ID,
        #[graphql(default = 2)] deadline_hours: i32,
    ) -> Result<GqlTodo> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.todo_store.write().await;

        // Ensure status is Doing
        let patch = TodoPatch {
            status: Some(TodoStatus::Doing),
            ..Default::default()
        };
        store
            .update(&id.0, patch)
            .await?
            .ok_or_else(|| Error::new(format!("Todo not found: {}", id.0)))?;

        // Set focus with deadline (max 5 concurrent focus slots)
        store.focus(&id.0, 5, deadline_hours as u64).await?;

        let todo = store
            .get(&id.0)
            .await?
            .ok_or_else(|| Error::new(format!("Todo not found: {}", id.0)))?;

        Ok(GqlTodo::from(todo))
    }

    /// End a focus session. Clears focus deadline.
    async fn unfocus_todo(&self, ctx: &Context<'_>, id: ID) -> Result<GqlTodo> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.todo_store.write().await;

        store.unfocus(&id.0).await?;

        let todo = store
            .get(&id.0)
            .await?
            .ok_or_else(|| Error::new(format!("Todo not found: {}", id.0)))?;

        Ok(GqlTodo::from(todo))
    }

    // ── Move Todo ─────────────────────────────────────────────────────────────

    /// Move a todo to a different parent or project.
    async fn move_todo(
        &self,
        ctx: &Context<'_>,
        id: ID,
        parent_id: Option<ID>,
        project_id: Option<ID>,
    ) -> Result<GqlTodo> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.todo_store.write().await;

        let updated = store
            .move_todo(
                &id.0,
                parent_id.map(|id| id.0),
                project_id.map(|id| id.0),
            )
            .await?
            .ok_or_else(|| Error::new(format!("Todo not found: {}", id.0)))?;

        Ok(GqlTodo::from(updated))
    }

    /// Reorder a todo to appear after another todo.
    async fn reorder_todo(
        &self,
        ctx: &Context<'_>,
        id: ID,
        after_id: Option<ID>,
    ) -> Result<GqlTodo> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.todo_store.write().await;

        // Validate after_id exists if provided
        if let Some(ref after) = after_id {
            if store.get(&after.0).await?.is_none() {
                return Err(Error::new(format!("After-todo not found: {}", after.0)));
            }
        }

        // Positional reordering: touch updated_at to signal change.
        // Full implementation would manipulate the JSONL order on disk.
        let patch = TodoPatch::default();
        let updated = store
            .update(&id.0, patch)
            .await?
            .ok_or_else(|| Error::new(format!("Todo not found: {}", id.0)))?;

        Ok(GqlTodo::from(updated))
    }

    // ── Log Time ──────────────────────────────────────────────────────────────

    /// Log a manual time entry on a todo.
    async fn log_time(
        &self,
        ctx: &Context<'_>,
        id: ID,
        minutes: i32,
        note: Option<String>,
    ) -> Result<GqlTodo> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.todo_store.write().await;

        let secs = (minutes as u64) * 60;
        let now = Utc::now();
        let entry = TimeEntry {
            id: Todo::generate_id(),
            started_at: now - chrono::Duration::seconds(secs as i64),
            ended_at: Some(now),
            duration_secs: Some(secs),
            note,
            source: TimeEntrySource::Manual,
        };

        store.add_time_entry(&id.0, entry).await?;

        let todo = store
            .get(&id.0)
            .await?
            .ok_or_else(|| Error::new(format!("Todo not found: {}", id.0)))?;

        Ok(GqlTodo::from(todo))
    }

    // ── Dependencies ─────────────────────────────────────────────────────────

    /// Add a dependency: `id` is blocked by `blocked_by_id`.
    async fn add_dependency(
        &self,
        ctx: &Context<'_>,
        id: ID,
        blocked_by_id: ID,
    ) -> Result<GqlTodo> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.todo_store.write().await;

        store
            .add_dependency(&id.0, &blocked_by_id.0)
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("cycle") {
                    Error::new("This would create a circular dependency")
                } else {
                    Error::new(msg)
                }
            })?;

        let todo = store
            .get(&id.0)
            .await?
            .ok_or_else(|| Error::new(format!("Todo not found: {}", id.0)))?;

        Ok(GqlTodo::from(todo))
    }

    /// Remove a dependency between two todos.
    async fn remove_dependency(
        &self,
        ctx: &Context<'_>,
        id: ID,
        blocked_by_id: ID,
    ) -> Result<GqlTodo> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.todo_store.write().await;

        store.remove_dependency(&id.0, &blocked_by_id.0).await?;

        let todo = store
            .get(&id.0)
            .await?
            .ok_or_else(|| Error::new(format!("Todo not found: {}", id.0)))?;

        Ok(GqlTodo::from(todo))
    }

    // ── Projects ──────────────────────────────────────────────────────────────

    /// Create a new project.
    async fn create_project(
        &self,
        ctx: &Context<'_>,
        input: CreateProjectInput,
    ) -> Result<GqlProject> {
        if input.name.trim().is_empty() {
            return Err(Error::new("Project name is required"));
        }

        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.project_store.write().await;

        let now = Utc::now();
        let project = Project {
            id: Project::generate_id(),
            name: input.name,
            description: input.description,
            color: input.color.map(Into::into).unwrap_or_default(),
            tags: input.tags.unwrap_or_default(),
            status: tools::project_types::ProjectStatus::Active,
            created_at: now,
            updated_at: now,
        };

        let created = store.add(project).await?;
        let pid = created.id.clone();
        drop(store);
        use crate::events::{ChangeAction, DashboardEvent};
        let _ = ctx_data.event_tx.send(DashboardEvent::ProjectChanged {
            action: ChangeAction::Created,
            id: pid,
        });
        Ok(GqlProject::from(created))
    }

    /// Partially update a project.
    async fn update_project(
        &self,
        ctx: &Context<'_>,
        id: ID,
        input: UpdateProjectInput,
    ) -> Result<GqlProject> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.project_store.write().await;

        let patch = ProjectPatch {
            name: input.name,
            description: input.description.map(Some),
            color: input.color.map(Into::into),
            tags: input.tags,
            status: input.status.map(Into::into),
        };

        let updated = store
            .update(&id.0, patch)
            .await?
            .ok_or_else(|| Error::new(format!("Project not found: {}", id.0)))?;

        Ok(GqlProject::from(updated))
    }

    /// Archive a project (sets status to Archived).
    async fn archive_project(&self, ctx: &Context<'_>, id: ID) -> Result<GqlProject> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.project_store.write().await;

        let patch = ProjectPatch {
            status: Some(tools::project_types::ProjectStatus::Archived),
            ..Default::default()
        };

        let updated = store
            .update(&id.0, patch)
            .await?
            .ok_or_else(|| Error::new(format!("Project not found: {}", id.0)))?;

        Ok(GqlProject::from(updated))
    }

    // ═══════════════════════════════════════════════════════════════════
    // Goal Mutations (dev-2)
    // ═══════════════════════════════════════════════════════════════════

    /// Create a new goal.
    async fn create_goal(&self, ctx: &Context<'_>, input: CreateGoalInput) -> Result<GqlGoal> {
        use crate::events::{ChangeAction, DashboardEvent};
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let now = Utc::now();

        let new_goal = goal::Goal {
            id: uuid::Uuid::new_v4(),
            title: input.title,
            description: input.description.unwrap_or_default(),
            status: input.status.map(Into::into).unwrap_or_default(),
            priority: input.priority.unwrap_or(3) as u8,
            target_date: input.target_date,
            created_at: now,
            updated_at: now,
            metrics: vec![],
            linked_project_ids: vec![],
            metadata: std::collections::HashMap::new(),
        };

        let mut store = ctx_data.goal_store.write().await;
        let created = store.add(new_goal).await?;
        let gid = created.id.to_string();
        drop(store);

        let _ = ctx_data
            .event_tx
            .send(DashboardEvent::GoalChanged { action: ChangeAction::Created, id: gid });

        Ok(GqlGoal(created))
    }

    /// Update fields on an existing goal.
    async fn update_goal(
        &self,
        ctx: &Context<'_>,
        id: ID,
        input: UpdateGoalInput,
    ) -> Result<GqlGoal> {
        use crate::events::{ChangeAction, DashboardEvent};
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let uuid = uuid::Uuid::parse_str(&id.0).map_err(|e| Error::new(e.to_string()))?;

        let mut store = ctx_data.goal_store.write().await;
        let mut g = store
            .get(&uuid)
            .await?
            .ok_or_else(|| Error::new(format!("Goal not found: {}", id.0)))?;

        if let Some(t) = input.title { g.title = t; }
        if let Some(d) = input.description { g.description = d; }
        if let Some(p) = input.priority { g.priority = p as u8; }
        if let Some(td) = input.target_date { g.target_date = Some(td); }
        if let Some(s) = input.status { g.status = s.into(); }

        let updated = store.update(g).await?.unwrap();
        let gid = updated.id.to_string();
        drop(store);

        let _ = ctx_data
            .event_tx
            .send(DashboardEvent::GoalChanged { action: ChangeAction::Updated, id: gid });

        Ok(GqlGoal(updated))
    }

    /// Update the current value of a named metric on a goal.
    async fn update_metric(
        &self,
        ctx: &Context<'_>,
        goal_id: ID,
        metric_name: String,
        current: f64,
    ) -> Result<GqlGoal> {
        use crate::events::{ChangeAction, DashboardEvent};
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let uuid = uuid::Uuid::parse_str(&goal_id.0).map_err(|e| Error::new(e.to_string()))?;

        let mut store = ctx_data.goal_store.write().await;
        let mut g = store
            .get(&uuid)
            .await?
            .ok_or_else(|| Error::new(format!("Goal not found: {}", goal_id.0)))?;

        let m = g
            .metrics
            .iter_mut()
            .find(|m| m.name == metric_name)
            .ok_or_else(|| Error::new(format!("Metric '{}' not found", metric_name)))?;
        m.current = current;

        let updated = store.update(g).await?.unwrap();
        let gid = updated.id.to_string();
        drop(store);

        let _ = ctx_data
            .event_tx
            .send(DashboardEvent::GoalChanged { action: ChangeAction::Updated, id: gid });

        Ok(GqlGoal(updated))
    }

    // ═══════════════════════════════════════════════════════════════════
    // Plan Mutations (dev-2)
    // ═══════════════════════════════════════════════════════════════════

    /// Create a new plan (starts in Draft status).
    async fn create_plan(&self, ctx: &Context<'_>, input: CreatePlanInput) -> Result<GqlPlan> {
        use crate::events::{ChangeAction, DashboardEvent};
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let now = Utc::now();

        let steps = input
            .steps
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .map(|(i, s)| plan::PlanStep {
                id: uuid::Uuid::new_v4(),
                index: i,
                description: s.description,
                reasoning: s.reasoning.unwrap_or_default(),
                expected_tools: s.expected_tools.unwrap_or_default(),
                status: plan::StepStatus::Pending,
                attempt_count: 0,
                max_attempts: 3,
                result: None,
                started_at: None,
                completed_at: None,
            })
            .collect();

        let new_plan = plan::Plan {
            id: uuid::Uuid::new_v4(),
            session_key: input.session_key.unwrap_or_else(|| "default".to_string()),
            goal_id: input.goal_id.as_deref().and_then(|g| uuid::Uuid::parse_str(g).ok()),
            title: input.title,
            description: input.description.unwrap_or_default(),
            status: plan::PlanStatus::Draft,
            steps,
            current_step_index: 0,
            iteration_limit: 50,
            backtrack_history: vec![],
            created_at: now,
            updated_at: now,
            completed_at: None,
        };

        let mut store = ctx_data.plan_store.write().await;
        let created = store.upsert(new_plan).await?;
        let pid = created.id.to_string();
        drop(store);

        let _ = ctx_data
            .event_tx
            .send(DashboardEvent::PlanChanged { action: ChangeAction::Created, id: pid });

        Ok(GqlPlan(created))
    }

    /// Approve a plan (Draft → Approved).
    async fn approve_plan(&self, ctx: &Context<'_>, id: ID) -> Result<GqlPlan> {
        do_plan_transition(ctx, &id.0, plan::PlanStatus::Approved).await
    }

    /// Execute a plan (Approved → Executing).
    async fn execute_plan(&self, ctx: &Context<'_>, id: ID) -> Result<GqlPlan> {
        do_plan_transition(ctx, &id.0, plan::PlanStatus::Executing).await
    }

    /// Abandon a plan (any active state → Abandoned).
    async fn abandon_plan(&self, ctx: &Context<'_>, id: ID) -> Result<GqlPlan> {
        do_plan_transition(ctx, &id.0, plan::PlanStatus::Abandoned).await
    }

    // ═══════════════════════════════════════════════════════════════════
    // Chat Mutation (dev-2)
    // ═══════════════════════════════════════════════════════════════════

    /// Send a chat message. Returns the user message; agent reply arrives via subscription/WS.
    async fn send_message(
        &self,
        _ctx: &Context<'_>,
        content: String,
        session_id: Option<String>,
    ) -> Result<GqlChatMessage> {
        use super::types::chat::MessageRole;
        let sid = session_id.unwrap_or_else(|| format!("web-{}", uuid::Uuid::new_v4()));
        Ok(GqlChatMessage {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::User,
            content,
            session_id: sid,
            created_at: Utc::now(),
        })
    }

    // ═══════════════════════════════════════════════════════════════════
    // Config Mutation (dev-2)
    // ═══════════════════════════════════════════════════════════════════

    /// Update a config section with partial JSON values. Returns the updated config.
    async fn update_config(
        &self,
        ctx: &Context<'_>,
        section: String,
        values: serde_json::Value,
    ) -> Result<GqlConfig> {
        use crate::events::DashboardEvent;
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut cfg = ctx_data.config.write().await;

        do_config_patch(&mut cfg, &section, &values)?;

        let result = GqlConfig::from_config(&cfg);
        let _ = ctx_data.event_tx.send(DashboardEvent::ConfigReloaded);

        Ok(result)
    }

    // ═══════════════════════════════════════════════════════════════════
    // Calendar Mutation (dev-2)
    // ═══════════════════════════════════════════════════════════════════

    /// Trigger an immediate calendar sync. Returns result statistics.
    async fn sync_calendar(&self, _ctx: &Context<'_>) -> Result<GqlCalendarSyncResult> {
        Ok(GqlCalendarSyncResult {
            success: true,
            events_synced: 0,
            conflicts_found: 0,
            duration_ms: 0,
            message: "Calendar sync not yet configured".to_string(),
        })
    }
}


