//! SQLite storage layer for klyntbot.
//!
//! Provides connection pooling with automatic migrations, row structs
//! for `sqlx::FromRow` deserialization, and a `Repos` aggregate for
//! convenient access to all repositories.

#[macro_use]
mod macros;

pub mod error;
pub mod finance_storage;
pub mod pool;
pub mod repos;
pub mod rows;
pub mod vector_store;

pub use error::{OptionExt, StorageError};
pub use pool::StoragePool;
pub use repos::Repos;
pub use vector_store::VectorStore;

// Re-export repo types for consumer convenience.
pub use repos::action_repo::{ActionFilter, ActionPatch, ActionRepo, ActionSummary};
pub use repos::area::AreaRepo;
pub use repos::key_result::KeyResultRepo;
pub use repos::objective::ObjectiveRepo;
pub use repos::project_repo::{ProjectFilter, ProjectPatch, ProjectRepo, ProjectWithStats};
pub use repos::AgentTaskRepo;
pub use repos::CalendarEventCacheRepo;
pub use repos::CalendarSyncRepo;
pub use repos::CronRepo;
pub use repos::DecisionLogRepo;
pub use repos::LearningStateRepo;
pub use repos::MemoryNoteRepo;
pub use repos::OutcomeRepo;
pub use repos::SessionRepo;
pub use repos::UsageRepo;
pub use repos::{OverallStats, StrategyRepo, ToolStatsRow};

// Re-export finance types.
pub use finance_storage::FinanceStorage;
pub use repos::FinanceAccountRepo;
pub use repos::FinanceBudgetRepo;
pub use repos::FinanceGoalRepo;
pub use repos::FinanceInvestmentRepo;
pub use repos::FinanceLiabilityRepo;
pub use repos::FinanceTransactionRepo;

// Re-export row structs for consumer convenience.
pub use rows::action::{ActionAttachmentRow, ActionDependencyRow, ActionRow, ActionTimeEntryRow};
pub use rows::agent_task::AgentTaskRow;
pub use rows::area::AreaRow;
pub use rows::calendar::{CalendarEventCacheRow, CalendarSyncStateRow};
pub use rows::cron::CronJobRow;
pub use rows::key_result::KeyResultRow;
pub use rows::learning::{
    DecisionLogRow, EnrichmentFeedbackRow, LearningStateRow, OutcomeRow, StrategyRecordRow,
    StrategySummaryRow,
};
pub use rows::memory::MemoryNoteRow;
pub use rows::objective::ObjectiveRow;
pub use rows::project::ProjectRow;
pub use rows::session::{SessionListRow, SessionMessageRow, SessionRow};
pub use rows::usage::UsageRecordRow;
