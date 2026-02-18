//! GraphQL schema for the Klyntbot web dashboard.
//!
//! Exports:
//! - `DashboardContext` — resolver context holding shared stores and event bus
//! - `build_schema()` — constructs the schema builder
//! - `QueryRoot`, `MutationRoot`, `SubscriptionRoot` — schema entry points

pub mod filters;
pub mod mutation;
pub mod query;
pub mod subscription;
pub mod types;

pub use mutation::MutationRoot;
pub use query::{GoalPlanSystemQueries, QueryRoot, TodoProjectQueries};
pub use subscription::SubscriptionRoot;

use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use config::Config;
use goal::GoalStore;
use plan::PlanStore;
use tools::project_store::ProjectStore;
use tools::todo_store::TodoStore;

use crate::events::DashboardEvent;

/// Shared context passed to every GraphQL resolver.
///
/// Injected into the schema at construction time via `.data(ctx)`.
/// Resolvers access it with `ctx.data_unchecked::<DashboardContext>()`.
#[derive(Clone)]
pub struct DashboardContext {
    pub todo_store: Arc<RwLock<TodoStore>>,
    pub project_store: Arc<RwLock<ProjectStore>>,
    pub goal_store: Arc<RwLock<GoalStore>>,
    pub plan_store: Arc<RwLock<PlanStore>>,
    pub event_tx: broadcast::Sender<DashboardEvent>,
    pub config: Arc<RwLock<Config>>,
}

/// Type alias for the full dashboard schema.
pub type DashboardSchema = async_graphql::Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

/// Build and return the schema builder with registered root types.
///
/// Usage: `build_schema().data(ctx).finish()`
pub fn build_schema() -> async_graphql::SchemaBuilder<QueryRoot, MutationRoot, SubscriptionRoot> {
    async_graphql::Schema::build(
        QueryRoot::default(),
        MutationRoot::default(),
        SubscriptionRoot,
    )
}
