//! Repository modules and aggregate struct.

pub mod action_repo;
pub mod agent_task;
pub mod area;
pub mod coaching_strategy;
pub mod cron;
pub mod custom_column;
pub mod decision_log;
pub mod entity_link_repo;

pub mod finance_account_repo;
pub mod finance_allocation_repo;
pub mod finance_budget_repo;
pub mod finance_exchange_rate_repo;
pub mod finance_goal_repo;
pub mod finance_investment_repo;
pub mod finance_liability_repo;
pub mod finance_snapshot_repo;
pub mod finance_transaction_repo;
pub mod interaction_log;
pub mod key_result;
pub mod learning_state;
pub mod objective;
pub mod outcome;
pub mod project_repo;
pub mod project_source_repo;
pub mod session;
pub mod session_context;
pub mod status_workflow;
pub mod strategy;
pub mod task_group;
pub mod task_repo;
#[cfg(test)]
pub mod tests;
pub mod tool_usage;
pub mod usage;

pub use action_repo::{ActionFilter, ActionPatch, ActionRepo, ActionSummary};
pub use agent_task::AgentTaskRepo;
pub use area::AreaRepo;
pub use coaching_strategy::{CoachingStrategyRepo, CoachingStrategyRow, UpsertCoachingStrategy};
pub use cron::CronRepo;
pub use custom_column::CustomColumnRepo;
pub use decision_log::DecisionLogRepo;
pub use entity_link_repo::EntityLinkRepo;
pub use finance_account_repo::FinanceAccountRepo;
pub use finance_allocation_repo::FinanceAllocationRepo;
pub use finance_budget_repo::FinanceBudgetRepo;
pub use finance_exchange_rate_repo::FinanceExchangeRateRepo;
pub use finance_goal_repo::FinanceGoalRepo;
pub use finance_investment_repo::FinanceInvestmentRepo;
pub use finance_liability_repo::FinanceLiabilityRepo;
pub use finance_snapshot_repo::FinanceSnapshotRepo;
pub use finance_transaction_repo::FinanceTransactionRepo;
pub use interaction_log::InteractionLogRepo;
pub use key_result::KeyResultRepo;
pub use learning_state::LearningStateRepo;
pub use objective::ObjectiveRepo;
pub use outcome::OutcomeRepo;
pub use project_repo::{ProjectFilter, ProjectPatch, ProjectRepo, ProjectWithStats};
pub use project_source_repo::ProjectSourceRepo;
pub use session::SessionRepo;
pub use session_context::{SessionContextParams, SessionContextRepo};
pub use status_workflow::StatusWorkflowRepo;
pub use strategy::{OverallStats, StrategyRepo, ToolStatsRow};
pub use task_group::TaskGroupRepo;
pub use task_repo::{TaskFilter, TaskPatch, TaskRepo, TaskSummary};
pub use tool_usage::ToolUsageRepo;
pub use usage::UsageRepo;

/// Aggregate of all repository handles, constructed from a single `SqlitePool`.
///
/// All repos are `Clone + Send + Sync` (wrapping `SqlitePool` which is `Arc`-based).
#[derive(Debug, Clone)]
pub struct Repos {
    pool: sqlx::SqlitePool,
    pub agent_tasks: AgentTaskRepo,
    pub actions: ActionRepo,
    pub areas: AreaRepo,
    pub projects: ProjectRepo,
    pub sessions: SessionRepo,
    pub objectives: ObjectiveRepo,
    pub key_results: KeyResultRepo,
    pub outcomes: OutcomeRepo,
    pub strategies: StrategyRepo,
    pub usage: UsageRepo,
    pub cron: CronRepo,
    pub learning_state: LearningStateRepo,
    pub decision_log: DecisionLogRepo,
    pub session_context: SessionContextRepo,
    pub finance: crate::FinanceStorage,
    pub interaction_log: InteractionLogRepo,
    pub status_workflows: StatusWorkflowRepo,
    pub task_groups: TaskGroupRepo,
    pub custom_columns: CustomColumnRepo,
    pub entity_links: EntityLinkRepo,
    pub project_sources: ProjectSourceRepo,
    pub tasks: TaskRepo,
    pub tool_usage: ToolUsageRepo,
}

impl Repos {
    /// Create all repository instances from a connected pool.
    pub fn from_pool(pool: &crate::pool::StoragePool) -> Self {
        let db = pool.inner().clone();
        Self {
            agent_tasks: AgentTaskRepo::new(db.clone()),
            actions: ActionRepo::new(db.clone()),
            areas: AreaRepo::new(db.clone()),
            projects: ProjectRepo::new(db.clone()),
            sessions: SessionRepo::new(db.clone()),
            objectives: ObjectiveRepo::new(db.clone()),
            key_results: KeyResultRepo::new(db.clone()),
            outcomes: OutcomeRepo::new(db.clone()),
            strategies: StrategyRepo::new(db.clone()),
            usage: UsageRepo::new(db.clone()),
            cron: CronRepo::new(db.clone()),
            learning_state: LearningStateRepo::new(db.clone()),
            decision_log: DecisionLogRepo::new(db.clone()),
            session_context: SessionContextRepo::new(db.clone()),
            finance: crate::FinanceStorage::from_pool(&db),
            interaction_log: InteractionLogRepo::new(db.clone()),
            status_workflows: StatusWorkflowRepo::new(db.clone()),
            task_groups: TaskGroupRepo::new(db.clone()),
            custom_columns: CustomColumnRepo::new(db.clone()),
            entity_links: EntityLinkRepo::new(db.clone()),
            project_sources: ProjectSourceRepo::new(db.clone()),
            tasks: TaskRepo::new(db.clone()),
            tool_usage: ToolUsageRepo::new(db.clone()),
            pool: db,
        }
    }

    /// Access the underlying pool (for repos that need direct access).
    pub fn pool(&self) -> &sqlx::SqlitePool {
        &self.pool
    }

    /// Run retention cleanup on analytics tables. Returns total rows deleted.
    ///
    /// Default retention periods:
    /// - `strategy_records`: 90 days
    /// - `learning_outcomes`: 30 days
    /// - `interaction_log`: 60 days
    /// - `tool_usage`: 90 days
    /// - `enrichment_feedback`: 90 days
    pub async fn cleanup_analytics(&self) -> Result<u64, crate::error::StorageError> {
        let now = chrono::Utc::now();
        let mut total = 0u64;

        total += self.strategies.delete_older_than(90, now).await?;
        total += self.outcomes.delete_older_than(30, now).await?;
        total += self.interaction_log.delete_older_than(60, now).await?;
        total += self.tool_usage.delete_older_than(90, now).await?;

        // enrichment_feedback
        let cutoff_90 = now - chrono::Duration::days(90);
        let result = sqlx::query("DELETE FROM enrichment_feedback WHERE timestamp < ?1")
            .bind(cutoff_90)
            .execute(&self.pool)
            .await?;
        total += result.rows_affected();

        Ok(total)
    }
}
