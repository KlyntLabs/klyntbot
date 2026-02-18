//! Root Query resolver — merged from todo/project (dev-1) and goal/plan/system (dev-2).
//!
//! Uses `MergedObject` to combine resolvers from both dev-1 and dev-2 domains
//! into a single `QueryRoot` without conflicting `#[Object]` implementations.

use async_graphql::{Context, MergedObject, Object, Result, ID};

use tools::project_types::ProjectFilter;
use tools::todo_types::TodoFilter;

use super::filters::{ProjectFilterInput, TodoFilterInput};
use super::types::project::GqlProject;
use super::types::todo::{
    GqlSearchMode, GqlSearchResult, GqlTodo, GqlTodoConnection, GqlTodoNode, GqlTodoStatusCount,
    GqlTodoSummary,
};
use super::DashboardContext;

// ── dev-1: Todo & Project queries ─────────────────────────────────────────────

/// Todo and Project query resolvers (dev-1).
#[derive(Default)]
pub struct TodoProjectQueries;

#[Object]
impl TodoProjectQueries {
    /// Paginated list of todos with optional filtering.
    async fn todos(
        &self,
        ctx: &Context<'_>,
        filter: Option<TodoFilterInput>,
        #[graphql(default = 50)] limit: i32,
        #[graphql(default = 0)] offset: i32,
    ) -> Result<GqlTodoConnection> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.todo_store.write().await;

        let native_filter = filter.map(todo_filter_to_native).unwrap_or_default();
        let all = store.list(&native_filter).await?;
        let total_count = all.len() as i32;

        let nodes: Vec<GqlTodo> = all
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .map(GqlTodo::from)
            .collect();

        Ok(GqlTodoConnection { nodes, total_count })
    }

    /// Fetch a single todo by ID. Returns `null` if not found.
    async fn todo(&self, ctx: &Context<'_>, id: ID) -> Result<Option<GqlTodo>> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.todo_store.write().await;
        let todo = store.get(&id.0).await?;
        Ok(todo.map(GqlTodo::from))
    }

    /// Hierarchical tree of todos. Optionally filtered to a single project.
    async fn todo_tree(
        &self,
        ctx: &Context<'_>,
        project_id: Option<ID>,
        #[graphql(default = 3)] _depth: i32,
    ) -> Result<Vec<GqlTodoNode>> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.todo_store.write().await;

        let filter = TodoFilter {
            project_id: project_id.as_ref().map(|id| id.0.clone()),
            ..Default::default()
        };
        let all = store.list(&filter).await?;

        let ids: std::collections::HashSet<String> = all.iter().map(|t| t.id.clone()).collect();
        let roots: Vec<_> = all
            .iter()
            .filter(|t| {
                t.parent_id
                    .as_ref()
                    .map(|pid| !ids.contains(pid))
                    .unwrap_or(true)
            })
            .cloned()
            .collect();

        let nodes = roots
            .into_iter()
            .map(|root| build_tree_node(root, &all))
            .collect();

        Ok(nodes)
    }

    /// Aggregate statistics for the todo collection.
    async fn todo_summary(&self, ctx: &Context<'_>) -> Result<GqlTodoSummary> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.todo_store.write().await;
        let summary = store.summary().await?;

        let by_status = super::types::todo::GqlTodoStatus::all_variants()
            .iter()
            .map(|&status| {
                let native: tools::todo_types::TodoStatus = status.into();
                let count = summary.by_status.get(&native).copied().unwrap_or(0) as i32;
                GqlTodoStatusCount { status, count }
            })
            .collect();

        let focus_active = {
            let focused = store.focused().await?;
            !focused.is_empty()
        };

        Ok(GqlTodoSummary {
            total: summary.total as i32,
            by_status,
            overdue_count: summary.overdue.len() as i32,
            upcoming_week_count: summary.upcoming_week.len() as i32,
            focus_active,
        })
    }

    /// Keyword, semantic, or hybrid search across todos.
    async fn search_todos(
        &self,
        ctx: &Context<'_>,
        query: String,
        #[graphql(default)] mode: GqlSearchMode,
    ) -> Result<Vec<GqlSearchResult>> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.todo_store.write().await;

        let results = match mode {
            GqlSearchMode::Keyword => {
                let q = query.to_lowercase();
                let filter = TodoFilter::default();
                store
                    .list(&filter)
                    .await?
                    .into_iter()
                    .filter(|t| {
                        t.title.to_lowercase().contains(&q)
                            || t.description
                                .as_ref()
                                .map(|d| d.to_lowercase().contains(&q))
                                .unwrap_or(false)
                    })
                    .map(|t| GqlSearchResult {
                        todo: GqlTodo::from(t),
                        score: None,
                        match_type: GqlSearchMode::Keyword,
                    })
                    .collect()
            }
            GqlSearchMode::Semantic | GqlSearchMode::Hybrid => {
                // Placeholder — embedding-backed search wired in Phase 2
                vec![]
            }
        };

        Ok(results)
    }

    /// List all projects with optional status/tag filter.
    async fn projects(
        &self,
        ctx: &Context<'_>,
        filter: Option<ProjectFilterInput>,
        #[graphql(default = 20)] limit: i32,
    ) -> Result<Vec<GqlProject>> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.project_store.write().await;

        let native_filter = filter.map(project_filter_to_native).unwrap_or_default();
        let projects = store.list(&native_filter).await?;

        Ok(projects
            .into_iter()
            .take(limit as usize)
            .map(GqlProject::from)
            .collect())
    }

    /// Fetch a single project by ID. Returns `null` if not found.
    async fn project(&self, ctx: &Context<'_>, id: ID) -> Result<Option<GqlProject>> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.project_store.write().await;
        let project = store.get(&id.0).await?;
        Ok(project.map(GqlProject::from))
    }
}

// ── dev-2: Goals, Plans, Config, System queries ───────────────────────────────

use super::types::config::GqlConfig;
use super::types::goal::{GoalFilter, GqlGoal};
use super::types::plan::{GqlPlan, PlanFilter};
use super::types::system::{GqlCronJob, GqlSession, GqlSystemStatus};

/// Goal, Plan, Config, and System query resolvers (dev-2).
#[derive(Default)]
pub struct GoalPlanSystemQueries;

#[Object]
impl GoalPlanSystemQueries {
    /// List all goals, optionally filtered by status.
    async fn goals(&self, ctx: &Context<'_>, filter: Option<GoalFilter>) -> Result<Vec<GqlGoal>> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.goal_store.write().await;
        let status = filter.and_then(|f| f.status).map(Into::into);
        let goals = store.list(status).await?;
        Ok(goals.into_iter().map(GqlGoal).collect())
    }

    /// Fetch a single goal by ID. Returns `null` if not found.
    async fn goal(&self, ctx: &Context<'_>, id: ID) -> Result<Option<GqlGoal>> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.goal_store.write().await;
        let uuid =
            uuid::Uuid::parse_str(&id.0).map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(store.get(&uuid).await?.map(GqlGoal))
    }

    /// List all plans, optionally filtered by status or session key.
    async fn plans(&self, ctx: &Context<'_>, filter: Option<PlanFilter>) -> Result<Vec<GqlPlan>> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.plan_store.write().await;

        let all = if let Some(f) = &filter {
            if let Some(sk) = &f.session_key {
                store
                    .get_active_plan(sk)
                    .await?
                    .map(|p| vec![p])
                    .unwrap_or_default()
            } else {
                store.all().await?
            }
        } else {
            store.all().await?
        };

        let filtered: Vec<GqlPlan> = all
            .into_iter()
            .filter(|p| {
                filter
                    .as_ref()
                    .and_then(|f| f.status)
                    .map(|s| {
                        let domain: plan::PlanStatus = s.into();
                        p.status == domain
                    })
                    .unwrap_or(true)
            })
            .map(GqlPlan)
            .collect();

        Ok(filtered)
    }

    /// Fetch a single plan by ID. Returns `null` if not found.
    async fn plan(&self, ctx: &Context<'_>, id: ID) -> Result<Option<GqlPlan>> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let mut store = ctx_data.plan_store.write().await;
        let uuid =
            uuid::Uuid::parse_str(&id.0).map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(store.get(&uuid).await?.map(GqlPlan))
    }

    /// Current configuration (hot-reloadable sections only).
    async fn config(&self, ctx: &Context<'_>) -> Result<GqlConfig> {
        let ctx_data = ctx.data_unchecked::<DashboardContext>();
        let cfg = ctx_data.config.read().await;
        Ok(GqlConfig::from_config(&cfg))
    }

    /// Live system status snapshot.
    async fn system_status(&self, _ctx: &Context<'_>) -> Result<GqlSystemStatus> {
        Ok(GqlSystemStatus {
            agent_running: true,
            uptime_secs: 0,
            connected_channels: vec![],
            calendar_sync_active: false,
            active_sessions: 0,
            version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }

    /// Configured cron jobs with schedule and last-run info.
    async fn cron_jobs(&self, _ctx: &Context<'_>) -> Result<Vec<GqlCronJob>> {
        Ok(vec![])
    }

    /// Recent chat sessions.
    async fn sessions(
        &self,
        _ctx: &Context<'_>,
        #[graphql(default = 10)] _limit: i32,
    ) -> Result<Vec<GqlSession>> {
        Ok(vec![])
    }
}

// ── Merged QueryRoot ──────────────────────────────────────────────────────────

/// Root query resolver — merges todo/project and goal/plan/system domains.
#[derive(MergedObject, Default)]
pub struct QueryRoot(pub TodoProjectQueries, pub GoalPlanSystemQueries);

// ── Helpers ───────────────────────────────────────────────────────────────────

fn todo_filter_to_native(f: TodoFilterInput) -> TodoFilter {
    TodoFilter {
        status: f.status.map(Into::into),
        priority_min: f.priority_min.map(|p| p as u8),
        tag: f.tag,
        project_id: f.project_id.map(|id| id.0),
        parent_id: f.parent_id.map(|id| id.0),
        ..Default::default()
    }
}

fn project_filter_to_native(f: ProjectFilterInput) -> ProjectFilter {
    ProjectFilter {
        status: f.status.map(Into::into),
        tag: f.tag,
        ..Default::default()
    }
}

fn build_tree_node(root: tools::todo_types::Todo, all: &[tools::todo_types::Todo]) -> GqlTodoNode {
    let root_id = root.id.clone();
    let children: Vec<GqlTodoNode> = all
        .iter()
        .filter(|t| t.parent_id.as_deref() == Some(&root_id))
        .cloned()
        .map(|child| build_tree_node(child, all))
        .collect();

    GqlTodoNode {
        todo: GqlTodo::from(root),
        children,
    }
}

impl super::types::todo::GqlTodoStatus {
    pub fn all_variants() -> &'static [Self] {
        use super::types::todo::GqlTodoStatus::*;
        &[Todo, Doing, Done, Archived]
    }
}
