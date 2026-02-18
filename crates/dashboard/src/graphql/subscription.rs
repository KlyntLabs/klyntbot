//! SubscriptionRoot — real-time event streams for the dashboard.
//!
//! Each resolver subscribes to the shared `broadcast::Sender<DashboardEvent>`
//! stored in `DashboardContext`, filters for the relevant variant, and yields
//! typed event structs to the GraphQL client over WebSocket.
//!
//! Pattern:
//!   1. Clone `DashboardContext` (all fields are `Arc<...>` / `Copy`)
//!   2. Call `event_tx.subscribe()` to get an independent broadcast receiver
//!   3. Use `async_stream::stream!` to convert the recv loop into a `Stream`
//!   4. Filter for the matching `DashboardEvent` variant and yield event structs

use async_graphql::{Context, Enum, SimpleObject, Subscription};
use futures_util::Stream;
use tokio::sync::broadcast;

use crate::events::{ChangeAction, DashboardEvent};
use super::DashboardContext;
use super::types::config::GqlConfig;
use super::types::goal::GqlGoal;
use super::types::plan::GqlPlan;
use super::types::project::GqlProject;
use super::types::system::GqlSystemStatus;
use super::types::todo::GqlTodo;

// ── Change action enum ────────────────────────────────────────────────────────

/// Whether a store entity was created, updated, or deleted.
#[derive(Enum, Clone, Copy, PartialEq, Eq)]
pub enum GqlChangeAction {
    Created,
    Updated,
    Deleted,
}

impl From<&ChangeAction> for GqlChangeAction {
    fn from(a: &ChangeAction) -> Self {
        match a {
            ChangeAction::Created => GqlChangeAction::Created,
            ChangeAction::Updated => GqlChangeAction::Updated,
            ChangeAction::Deleted => GqlChangeAction::Deleted,
        }
    }
}

// ── Subscription event payloads ───────────────────────────────────────────────

/// Emitted when a todo item is created, updated, or deleted.
#[derive(SimpleObject)]
pub struct GqlTodoChangeEvent {
    /// What happened to the todo.
    pub action: GqlChangeAction,
    /// ID of the affected todo.
    pub id: String,
    /// Full todo data (None when action = Deleted).
    pub todo: Option<GqlTodo>,
}

/// Emitted when a project is created, updated, or deleted.
#[derive(SimpleObject)]
pub struct GqlProjectChangeEvent {
    pub action: GqlChangeAction,
    pub id: String,
    /// Full project data (None when action = Deleted).
    pub project: Option<GqlProject>,
}

/// Emitted when a goal is created, updated, or a metric changes.
#[derive(SimpleObject)]
pub struct GqlGoalChangeEvent {
    pub action: GqlChangeAction,
    pub id: String,
    /// Full goal data (None when action = Deleted).
    pub goal: Option<GqlGoal>,
}

/// Emitted when a plan transitions state, a step completes, or backtrack occurs.
#[derive(SimpleObject)]
pub struct GqlPlanChangeEvent {
    pub action: GqlChangeAction,
    pub id: String,
    /// Full plan data (None when action = Deleted).
    pub plan: Option<GqlPlan>,
}

/// A push notification (task overdue, agent message, reminder, etc.)
#[derive(SimpleObject, Clone)]
pub struct GqlNotification {
    pub title: String,
    pub body: String,
}

// ── SubscriptionRoot ──────────────────────────────────────────────────────────

pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    /// Real-time todo change events (created, updated, deleted).
    async fn todo_changed(
        &self,
        ctx: &Context<'_>,
    ) -> impl Stream<Item = GqlTodoChangeEvent> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>().clone();
        let mut rx = ctx_data.event_tx.subscribe();

        async_stream::stream! {
            loop {
                match rx.recv().await {
                    Ok(DashboardEvent::TodoChanged { action, id }) => {
                        let gql_action = GqlChangeAction::from(&action);
                        let todo = {
                            let mut store = ctx_data.todo_store.write().await;
                            store.get(&id).await.ok().flatten().map(GqlTodo::from)
                        };
                        yield GqlTodoChangeEvent { action: gql_action, id, todo };
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    _ => {} // other event types or Lagged — skip
                }
            }
        }
    }

    /// Real-time project change events.
    async fn project_changed(
        &self,
        ctx: &Context<'_>,
    ) -> impl Stream<Item = GqlProjectChangeEvent> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>().clone();
        let mut rx = ctx_data.event_tx.subscribe();

        async_stream::stream! {
            loop {
                match rx.recv().await {
                    Ok(DashboardEvent::ProjectChanged { action, id }) => {
                        let gql_action = GqlChangeAction::from(&action);
                        let project = {
                            let mut store = ctx_data.project_store.write().await;
                            store.get(&id).await.ok().flatten().map(GqlProject::from)
                        };
                        yield GqlProjectChangeEvent { action: gql_action, id, project };
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    _ => {}
                }
            }
        }
    }

    /// Real-time goal change events (created, updated, metric changed).
    async fn goal_changed(
        &self,
        ctx: &Context<'_>,
    ) -> impl Stream<Item = GqlGoalChangeEvent> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>().clone();
        let mut rx = ctx_data.event_tx.subscribe();

        async_stream::stream! {
            loop {
                match rx.recv().await {
                    Ok(DashboardEvent::GoalChanged { action, id }) => {
                        let gql_action = GqlChangeAction::from(&action);
                        let goal = {
                            let uuid = uuid::Uuid::parse_str(&id).ok();
                            if let Some(uuid) = uuid {
                                let mut store = ctx_data.goal_store.write().await;
                                store.get(&uuid).await.ok().flatten().map(GqlGoal)
                            } else {
                                None
                            }
                        };
                        yield GqlGoalChangeEvent { action: gql_action, id, goal };
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    _ => {}
                }
            }
        }
    }

    /// Real-time plan change events (step completed, backtrack, status transition).
    async fn plan_changed(
        &self,
        ctx: &Context<'_>,
    ) -> impl Stream<Item = GqlPlanChangeEvent> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>().clone();
        let mut rx = ctx_data.event_tx.subscribe();

        async_stream::stream! {
            loop {
                match rx.recv().await {
                    Ok(DashboardEvent::PlanChanged { action, id }) => {
                        let gql_action = GqlChangeAction::from(&action);
                        let plan = {
                            let uuid = uuid::Uuid::parse_str(&id).ok();
                            if let Some(uuid) = uuid {
                                let mut store = ctx_data.plan_store.write().await;
                                store.get(&uuid).await.ok().flatten().map(GqlPlan)
                            } else {
                                None
                            }
                        };
                        yield GqlPlanChangeEvent { action: gql_action, id, plan };
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    _ => {}
                }
            }
        }
    }

    /// Push notifications (task overdue, reminders, agent alerts).
    async fn notification(
        &self,
        ctx: &Context<'_>,
    ) -> impl Stream<Item = GqlNotification> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>().clone();
        let mut rx = ctx_data.event_tx.subscribe();

        async_stream::stream! {
            loop {
                match rx.recv().await {
                    Ok(DashboardEvent::Notification { title, body }) => {
                        yield GqlNotification { title, body };
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    _ => {}
                }
            }
        }
    }

    /// Config hot-reload events — emits the full config after each `updateConfig`
    /// call or file-watcher reload.
    async fn config_changed(
        &self,
        ctx: &Context<'_>,
    ) -> impl Stream<Item = GqlConfig> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>().clone();
        let mut rx = ctx_data.event_tx.subscribe();

        async_stream::stream! {
            loop {
                match rx.recv().await {
                    Ok(DashboardEvent::ConfigReloaded) => {
                        let cfg = ctx_data.config.read().await;
                        yield GqlConfig::from_config(&cfg);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    _ => {}
                }
            }
        }
    }

    /// Live system-status updates — emits a snapshot on each agent event
    /// (channel connect/disconnect, tool execution, etc.)
    async fn system_status(
        &self,
        ctx: &Context<'_>,
    ) -> impl Stream<Item = GqlSystemStatus> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>().clone();
        let mut rx = ctx_data.event_tx.subscribe();

        async_stream::stream! {
            loop {
                match rx.recv().await {
                    Ok(DashboardEvent::AgentEvent(_)) => {
                        yield GqlSystemStatus {
                            agent_running: true,
                            uptime_secs: 0,
                            connected_channels: vec![],
                            calendar_sync_active: false,
                            active_sessions: 0,
                            version: env!("CARGO_PKG_VERSION").to_string(),
                        };
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    _ => {}
                }
            }
        }
    }
}
