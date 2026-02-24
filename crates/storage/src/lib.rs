//! SQLite storage layer for klyntbot.
//!
//! Provides connection pooling with automatic migrations, row structs
//! for `sqlx::FromRow` deserialization, and a `Repos` aggregate for
//! convenient access to all repositories.

pub mod error;
pub mod pool;
pub mod repos;
pub mod rows;
pub mod vector_store;

pub use error::StorageError;
pub use pool::StoragePool;
pub use repos::Repos;
pub use vector_store::VectorStore;

// Re-export repo types for consumer convenience.
pub use repos::project_repo::{ProjectFilter, ProjectPatch, ProjectRepo, ProjectWithStats};
pub use repos::todo_repo::{TodoFilter, TodoPatch, TodoRepo, TodoSummary};
pub use repos::CalendarEventCacheRepo;
pub use repos::CalendarSyncRepo;
pub use repos::CronRepo;
pub use repos::DecisionLogRepo;
pub use repos::GoalRepo;
pub use repos::LearningStateRepo;
pub use repos::MemoryNoteRepo;
pub use repos::OutcomeRepo;
pub use repos::PlanRepo;
pub use repos::SessionRepo;
pub use repos::{OverallStats, StrategyRepo, ToolStatsRow};
pub use repos::UsageRepo;

// Re-export finance repo types.
pub use repos::FinanceAccountRepo;
pub use repos::FinanceBudgetRepo;
pub use repos::FinanceGoalRepo;
pub use repos::FinanceInvestmentRepo;
pub use repos::FinanceLiabilityRepo;
pub use repos::FinanceTransactionRepo;

// Re-export row structs for consumer convenience.
pub use rows::calendar::{CalendarEventCacheRow, CalendarSyncStateRow};
pub use rows::cron::CronJobRow;
pub use rows::goal::{GoalProjectLinkRow, GoalRow};
pub use rows::learning::{
    DecisionLogRow, EnrichmentFeedbackRow, LearningStateRow, OutcomeRow, StrategyRecordRow,
    StrategySummaryRow,
};
pub use rows::memory::MemoryNoteRow;
pub use rows::plan::{PlanRow, PlanStepRow};
pub use rows::project::ProjectRow;
pub use rows::session::{SessionListRow, SessionMessageRow, SessionRow};
pub use rows::todo::{TodoAttachmentRow, TodoDependencyRow, TodoRow, TodoTimeEntryRow};
pub use rows::usage::UsageRecordRow;
