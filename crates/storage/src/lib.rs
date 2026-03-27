//! SQLite storage layer for klyntbot.
//!
//! Provides connection pooling with automatic migrations, row structs
//! for `sqlx::FromRow` deserialization, and a `Repos` aggregate for
//! convenient access to all repositories.

#[macro_use]
mod macros;

pub mod circuit_breaker;
pub mod error;
pub mod finance_storage;
pub mod pool;
pub mod repos;
pub mod rows;
pub mod vector_store;

// ── Core infrastructure ─────────────────────────────────────────────
pub use error::{OptionExt, StorageError};
pub use pool::StoragePool;
pub use repos::Repos;
pub use vector_store::{sanitize_predicate_value, CognitiveFactParams, VectorStore};

// ── DND Override ─────────────────────────────────────────────────────
pub use repos::{DndOverrideRepo, DndOverrideRow};

// ── Actions / Tasks / Projects ──────────────────────────────────────
pub use repos::{CustomColumnRepo, ItemSummary, TaskGroupRepo};
pub use repos::{EntityLinkRepo, ProjectSourceRepo};
pub use repos::{ProjectFilter, ProjectPatch, ProjectRepo, ProjectWithStats};
pub use repos::{TaskFilter, TaskPatch, TaskRepo, TaskSummary, TimeEntryWithTask};

// ── OKR ─────────────────────────────────────────────────────────────
pub use repos::{AreaRepo, KeyResultRepo, ObjectiveRepo};

// ── Sessions / Context ──────────────────────────────────────────────
pub use repos::{SessionContextParams, SessionContextRepo, SessionRepo};

// ── Autotuner ────────────────────────────────────────────────────────
pub use repos::TrialRepo;
pub use rows::trial::{ExperimentRow, ShadowLogRow, TrialRow};

// ── Agent / Learning ────────────────────────────────────────────────
pub use repos::StatusWorkflowRepo;
pub use repos::ToolUsageRepo;
pub use repos::{AgentTaskRepo, CronRepo, UsageRepo};
pub use repos::{CoachingInterventionLogRepo, InterventionLogRow};
pub use repos::{CoachingStrategyRepo, CoachingStrategyRow, UpsertCoachingStrategy};
pub use repos::{DecisionLogRepo, InteractionLogRepo, LearningStateRepo, OutcomeRepo};
pub use repos::{OverallStats, StrategyRepo, ToolStatsRow};

// ── Finance ─────────────────────────────────────────────────────────
pub use finance_storage::FinanceStorage;
pub use repos::{
    FinanceAccountRepo, FinanceAllocationRepo, FinanceBudgetRepo, FinanceExchangeRateRepo,
    FinanceGoalRepo, FinanceInvestmentRepo, FinanceLiabilityRepo, FinanceSnapshotRepo,
    FinanceTransactionRepo,
};

// ── Row structs ─────────────────────────────────────────────────────
pub use rows::agent_task::AgentTaskRow;
pub use rows::area::AreaRow;
pub use rows::cron::CronJobRow;
pub use rows::custom_column::{CustomColumnRow, CustomColumnValueRow};
pub use rows::entity_link::EntityLinkRow;
pub use rows::finance::FinanceInvestmentTxRow;
pub use rows::key_result::KeyResultRow;
pub use rows::learning::{
    DecisionLogRow, EnrichmentFeedbackRow, InteractionLogRow, LearningStateRow, OutcomeRow,
    StrategyRecordRow, StrategySummaryRow,
};
pub use rows::objective::ObjectiveRow;
pub use rows::project::ProjectRow;
pub use rows::project_source::ProjectSourceRow;
pub use rows::session::{SessionListRow, SessionMessageRow, SessionRow};
pub use rows::task::{
    TaskActivityRow, TaskAttachmentRow, TaskDecompositionRow, TaskDependencyRow, TaskEstimationRow,
    TaskExecutionRow, TaskRow, TaskSuggestionRow, TaskTimeEntryRow,
};
pub use rows::task_group::TaskGroupRow;
pub use rows::tool_usage::{ToolUsageRow, ToolUsageStatsRow};
pub use rows::usage::UsageRecordRow;
