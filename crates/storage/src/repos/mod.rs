//! Repository modules and aggregate struct.

pub mod action_repo;
pub mod agent_task;
pub mod area;
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
pub mod key_result;
pub mod learning_state;
pub mod memory_note;
pub mod objective;
pub mod outcome;
pub mod project_repo;
pub mod session;
pub mod strategy;
pub mod usage;

#[cfg(test)]
pub mod tests;

pub use action_repo::{ActionFilter, ActionPatch, ActionRepo, ActionSummary};
pub use agent_task::AgentTaskRepo;
pub use area::AreaRepo;
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
pub use key_result::KeyResultRepo;
pub use learning_state::LearningStateRepo;
pub use memory_note::MemoryNoteRepo;
pub use objective::ObjectiveRepo;
pub use outcome::OutcomeRepo;
pub use project_repo::{ProjectFilter, ProjectPatch, ProjectRepo, ProjectWithStats};
pub use session::SessionRepo;
pub use strategy::{OverallStats, StrategyRepo, ToolStatsRow};
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
    pub calendar_sync: CalendarSyncRepo,
    pub calendar_event_cache: CalendarEventCacheRepo,
    pub memory_notes: MemoryNoteRepo,
    pub learning_state: LearningStateRepo,
    pub decision_log: DecisionLogRepo,
    pub finance: crate::FinanceStorage,
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
            calendar_sync: CalendarSyncRepo::new(db.clone()),
            calendar_event_cache: CalendarEventCacheRepo::new(db.clone()),
            memory_notes: MemoryNoteRepo::new(db.clone()),
            learning_state: LearningStateRepo::new(db.clone()),
            decision_log: DecisionLogRepo::new(db.clone()),
            finance: crate::FinanceStorage::from_pool(&db),
            pool: db,
        }
    }

    /// Access the underlying pool (for repos that need direct access).
    pub fn pool(&self) -> &sqlx::SqlitePool {
        &self.pool
    }
}
