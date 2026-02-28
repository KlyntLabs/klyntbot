//! Repository modules and aggregate struct.

pub mod agent_task;
pub mod calendar_event_cache;
pub mod calendar_sync;
pub mod cron;
pub mod decision_log;
pub mod finance_account_repo;
pub mod finance_budget_repo;
pub mod finance_goal_repo;
pub mod finance_investment_repo;
pub mod finance_liability_repo;
pub mod finance_transaction_repo;
pub mod goal;
pub mod learning_state;
pub mod memory_note;
pub mod outcome;
pub mod plan;
pub mod project_repo;
pub mod session;
pub mod strategy;
pub mod todo_repo;
pub mod usage;

#[cfg(test)]
pub mod tests;

pub use agent_task::AgentTaskRepo;
pub use calendar_event_cache::CalendarEventCacheRepo;
pub use calendar_sync::CalendarSyncRepo;
pub use cron::CronRepo;
pub use decision_log::DecisionLogRepo;
pub use finance_account_repo::FinanceAccountRepo;
pub use finance_budget_repo::FinanceBudgetRepo;
pub use finance_goal_repo::FinanceGoalRepo;
pub use finance_investment_repo::FinanceInvestmentRepo;
pub use finance_liability_repo::FinanceLiabilityRepo;
pub use finance_transaction_repo::FinanceTransactionRepo;
pub use goal::GoalRepo;
pub use learning_state::LearningStateRepo;
pub use memory_note::MemoryNoteRepo;
pub use outcome::OutcomeRepo;
pub use plan::PlanRepo;
pub use project_repo::{ProjectFilter, ProjectPatch, ProjectRepo, ProjectWithStats};
pub use session::SessionRepo;
pub use strategy::{OverallStats, StrategyRepo, ToolStatsRow};
pub use todo_repo::{TodoFilter, TodoPatch, TodoRepo, TodoSummary};
pub use usage::UsageRepo;

/// Aggregate of all repository handles, constructed from a single `SqlitePool`.
///
/// All repos are `Clone + Send + Sync` (wrapping `SqlitePool` which is `Arc`-based).
#[derive(Debug, Clone)]
pub struct Repos {
    pool: sqlx::SqlitePool,
    pub agent_tasks: AgentTaskRepo,
    pub todos: TodoRepo,
    pub projects: ProjectRepo,
    pub sessions: SessionRepo,
    pub goals: GoalRepo,
    pub plans: PlanRepo,
    pub outcomes: OutcomeRepo,
    pub strategies: StrategyRepo,
    pub usage: UsageRepo,
    pub cron: CronRepo,
    pub calendar_sync: CalendarSyncRepo,
    pub calendar_event_cache: CalendarEventCacheRepo,
    pub memory_notes: MemoryNoteRepo,
    pub learning_state: LearningStateRepo,
    pub decision_log: DecisionLogRepo,
    // Finance repos
    pub finance_accounts: FinanceAccountRepo,
    pub finance_transactions: FinanceTransactionRepo,
    pub finance_budgets: FinanceBudgetRepo,
    pub finance_investments: FinanceInvestmentRepo,
    pub finance_goals: FinanceGoalRepo,
    pub finance_liabilities: FinanceLiabilityRepo,
}

impl Repos {
    /// Create all repository instances from a connected pool.
    pub fn from_pool(pool: &crate::pool::StoragePool) -> Self {
        let db = pool.inner().clone();
        Self {
            agent_tasks: AgentTaskRepo::new(db.clone()),
            todos: TodoRepo::new(db.clone()),
            projects: ProjectRepo::new(db.clone()),
            sessions: SessionRepo::new(db.clone()),
            goals: GoalRepo::new(db.clone()),
            plans: PlanRepo::new(db.clone()),
            outcomes: OutcomeRepo::new(db.clone()),
            strategies: StrategyRepo::new(db.clone()),
            usage: UsageRepo::new(db.clone()),
            cron: CronRepo::new(db.clone()),
            calendar_sync: CalendarSyncRepo::new(db.clone()),
            calendar_event_cache: CalendarEventCacheRepo::new(db.clone()),
            memory_notes: MemoryNoteRepo::new(db.clone()),
            learning_state: LearningStateRepo::new(db.clone()),
            decision_log: DecisionLogRepo::new(db.clone()),
            finance_accounts: FinanceAccountRepo::new(db.clone()),
            finance_transactions: FinanceTransactionRepo::new(db.clone()),
            finance_budgets: FinanceBudgetRepo::new(db.clone()),
            finance_investments: FinanceInvestmentRepo::new(db.clone()),
            finance_goals: FinanceGoalRepo::new(db.clone()),
            finance_liabilities: FinanceLiabilityRepo::new(db.clone()),
            pool: db,
        }
    }

    /// Access the underlying pool (for repos that need direct access).
    pub fn pool(&self) -> &sqlx::SqlitePool {
        &self.pool
    }
}
