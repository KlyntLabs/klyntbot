//! Repository modules and aggregate struct.

pub mod calendar_event_cache;
pub mod calendar_sync;
pub mod conv_embedding;
pub mod cron;
pub mod decision_log;
pub mod embedding;
pub mod finance_account_repo;
pub mod finance_budget_repo;
pub mod finance_goal_repo;
pub mod finance_investment_repo;
pub mod finance_liability_repo;
pub mod finance_transaction_repo;
pub mod goal;
pub mod learning_state;
pub mod memory_note;
pub mod memory_note_embedding;
pub mod outcome;
pub mod plan;
pub mod project_repo;
pub mod session;
pub mod strategy;
pub mod todo_repo;
pub mod usage;

#[cfg(test)]
pub mod tests;

pub use calendar_event_cache::CalendarEventCacheRepo;
pub use calendar_sync::CalendarSyncRepo;
pub use conv_embedding::ConvEmbeddingRepo;
pub use cron::CronRepo;
pub use decision_log::DecisionLogRepo;
pub use embedding::EmbeddingRepo;
pub use finance_account_repo::FinanceAccountRepo;
pub use finance_budget_repo::FinanceBudgetRepo;
pub use finance_goal_repo::FinanceGoalRepo;
pub use finance_investment_repo::FinanceInvestmentRepo;
pub use finance_liability_repo::FinanceLiabilityRepo;
pub use finance_transaction_repo::FinanceTransactionRepo;
pub use goal::GoalRepo;
pub use learning_state::LearningStateRepo;
pub use memory_note::MemoryNoteRepo;
pub use memory_note_embedding::{MemoryNoteEmbeddingRepo, MemoryNoteMatch};
pub use outcome::OutcomeRepo;
pub use plan::PlanRepo;
pub use project_repo::{ProjectFilter, ProjectPatch, ProjectRepo, ProjectWithStats};
pub use session::SessionRepo;
pub use strategy::StrategyRepo;
pub use todo_repo::{TodoFilter, TodoPatch, TodoRepo, TodoSummary};
pub use usage::UsageRepo;

/// Aggregate of all repository handles, constructed from a single `PgPool`.
///
/// All repos are `Clone + Send + Sync` (wrapping `PgPool` which is `Arc`-based).
#[derive(Debug, Clone)]
pub struct Repos {
    pool: sqlx::PgPool,
    pub todos: TodoRepo,
    pub projects: ProjectRepo,
    pub sessions: SessionRepo,
    pub goals: GoalRepo,
    pub plans: PlanRepo,
    pub embeddings: EmbeddingRepo,
    pub conv_embeddings: ConvEmbeddingRepo,
    pub outcomes: OutcomeRepo,
    pub strategies: StrategyRepo,
    pub usage: UsageRepo,
    pub cron: CronRepo,
    pub calendar_sync: CalendarSyncRepo,
    pub calendar_event_cache: CalendarEventCacheRepo,
    pub memory_notes: MemoryNoteRepo,
    pub memory_note_embeddings: MemoryNoteEmbeddingRepo,
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
        let pg = pool.inner().clone();
        Self {
            todos: TodoRepo::new(pg.clone()),
            projects: ProjectRepo::new(pg.clone()),
            sessions: SessionRepo::new(pg.clone()),
            goals: GoalRepo::new(pg.clone()),
            plans: PlanRepo::new(pg.clone()),
            embeddings: EmbeddingRepo::new(pg.clone()),
            conv_embeddings: ConvEmbeddingRepo::new(pg.clone()),
            outcomes: OutcomeRepo::new(pg.clone()),
            strategies: StrategyRepo::new(pg.clone()),
            usage: UsageRepo::new(pg.clone()),
            cron: CronRepo::new(pg.clone()),
            calendar_sync: CalendarSyncRepo::new(pg.clone()),
            calendar_event_cache: CalendarEventCacheRepo::new(pg.clone()),
            memory_notes: MemoryNoteRepo::new(pg.clone()),
            memory_note_embeddings: MemoryNoteEmbeddingRepo::new(pg.clone()),
            learning_state: LearningStateRepo::new(pg.clone()),
            decision_log: DecisionLogRepo::new(pg.clone()),
            finance_accounts: FinanceAccountRepo::new(pg.clone()),
            finance_transactions: FinanceTransactionRepo::new(pg.clone()),
            finance_budgets: FinanceBudgetRepo::new(pg.clone()),
            finance_investments: FinanceInvestmentRepo::new(pg.clone()),
            finance_goals: FinanceGoalRepo::new(pg.clone()),
            finance_liabilities: FinanceLiabilityRepo::new(pg.clone()),
            pool: pg,
        }
    }

    /// Access the underlying pool (for repos that need direct access).
    pub fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }
}
