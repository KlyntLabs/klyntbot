//! PostgreSQL storage layer for klyntbot.
//!
//! Provides connection pooling with automatic migrations, row structs
//! for `sqlx::FromRow` deserialization, and a `Repos` aggregate for
//! convenient access to all repositories.

pub mod error;
pub mod pool;
pub mod repos;
pub mod rows;

pub use error::StorageError;
pub use pool::StoragePool;
pub use repos::Repos;

// Re-export repo types for consumer convenience.
pub use repos::project_repo::{ProjectFilter, ProjectPatch, ProjectRepo, ProjectWithStats};
pub use repos::todo_repo::{TodoFilter, TodoPatch, TodoRepo, TodoSummary};
pub use repos::CalendarSyncRepo;
pub use repos::ConvEmbeddingRepo;
pub use repos::CronRepo;
pub use repos::EmbeddingRepo;
pub use repos::GoalRepo;
pub use repos::OutcomeRepo;
pub use repos::PlanRepo;
pub use repos::SessionRepo;
pub use repos::StrategyRepo;
pub use repos::UsageRepo;

// Re-export row structs for consumer convenience.
pub use rows::calendar::CalendarSyncStateRow;
pub use rows::cron::CronJobRow;
pub use rows::embedding::{ConvEmbeddingRow, EmbeddingRow};
pub use rows::goal::{GoalProjectLinkRow, GoalRow};
pub use rows::learning::{EnrichmentFeedbackRow, OutcomeRow, StrategyRecordRow};
pub use rows::plan::{PlanRow, PlanStepRow};
pub use rows::project::ProjectRow;
pub use rows::session::{SessionMessageRow, SessionRow};
pub use rows::todo::{TodoAttachmentRow, TodoDependencyRow, TodoRow, TodoTimeEntryRow};
pub use rows::usage::UsageRecordRow;
