//! Repository modules and aggregate struct.

pub mod agent_task;
pub mod area;
pub mod brain_signal;
pub mod coaching_intervention_log;
pub mod coaching_strategy;
pub mod coding_approval_history;
pub mod cron;
pub mod custom_column;
pub mod decision_log;
pub mod dnd_override;
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
pub mod held_notifications;
pub mod interaction_log;
pub mod key_result;
pub mod learning_state;
pub mod notification_log;
pub mod objective;
pub mod outcome;
pub mod project_repo;
pub mod project_source_repo;
pub mod reforge_state;
pub mod reforge_suggestion;
pub mod response_warning;
pub mod retrieval_feedback;
pub mod scheduled_fires;
pub mod session;
pub mod session_context;
pub mod session_memory;
pub mod skill_version;
pub mod status_workflow;
pub mod strategy;
pub mod task_alarms;
pub mod task_group;
pub mod task_recurrence;
pub mod task_repo;
#[cfg(test)]
pub mod tests;
pub mod tool_usage;
pub mod trial_repo;
pub mod usage;

pub use agent_task::AgentTaskRepo;
pub use area::AreaRepo;
pub use brain_signal::{BrainSignalFeedbackRepo, BrainSignalFeedbackRow};
pub use coaching_intervention_log::{CoachingInterventionLogRepo, InterventionLogRow};
pub use coaching_strategy::{CoachingStrategyRepo, CoachingStrategyRow, UpsertCoachingStrategy};
pub use coding_approval_history::{
    ApprovalHistorySummary, CodingApprovalHistoryRepo, HistoryEntry,
};
pub use cron::CronRepo;
pub use custom_column::CustomColumnRepo;
pub use decision_log::DecisionLogRepo;
pub use dnd_override::{DndOverrideRepo, DndOverrideRow};
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
pub use held_notifications::HeldNotificationsRepo;
pub use interaction_log::InteractionLogRepo;
pub use key_result::KeyResultRepo;
pub use learning_state::LearningStateRepo;
pub use notification_log::NotificationLogRepo;
pub use objective::ObjectiveRepo;
pub use outcome::OutcomeRepo;
pub use project_repo::{ProjectFilter, ProjectPatch, ProjectRepo, ProjectWithStats};
pub use project_source_repo::ProjectSourceRepo;
pub use reforge_state::ReforgeStateRepo;
pub use reforge_suggestion::ReforgeSuggestionRepo;
pub use response_warning::{ResponseWarningRepo, ResponseWarningRow};
pub use retrieval_feedback::RetrievalFeedbackRepo;
pub use scheduled_fires::ScheduledFiresRepo;
pub use session::SessionRepo;
pub use session_context::{SessionContextParams, SessionContextRepo};
pub use session_memory::SessionMemoryRepo;
pub use skill_version::SkillVersionRepo;
pub use status_workflow::StatusWorkflowRepo;
pub use strategy::{OverallStats, StrategyRepo, ToolStatsRow};
pub use task_alarms::TaskAlarmsRepo;
pub use task_group::TaskGroupRepo;
pub use task_recurrence::TaskRecurrenceRepo;
pub use task_repo::{TaskFilter, TaskPatch, TaskRepo, TimeEntryWithTask};
pub use tool_usage::ToolUsageRepo;
pub use trial_repo::TrialRepo;
pub use usage::UsageRepo;

/// Aggregate counts by status — shared between actions and tasks.
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemSummary {
    pub todo: i64,
    pub doing: i64,
    pub done: i64,
    pub total: i64,
}

/// Type alias for backwards compatibility.
pub type TaskSummary = ItemSummary;

/// Compute an [`ItemSummary`] from `(status, count)` rows.
pub fn compute_summary(rows: &[(String, i64)]) -> ItemSummary {
    let mut summary = ItemSummary::default();
    for (status, count) in rows {
        match status.as_str() {
            "todo" => summary.todo = *count,
            "doing" => summary.doing = *count,
            "done" => summary.done = *count,
            _ => {}
        }
        summary.total += count;
    }
    summary
}

/// Aggregate of all repository handles, constructed from a single `SqlitePool`.
///
/// All repos are `Clone + Send + Sync` (wrapping `SqlitePool` which is `Arc`-based).
#[derive(Debug, Clone)]
pub struct Repos {
    pool: sqlx::SqlitePool,
    pub agent_tasks: AgentTaskRepo,
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
    pub session_memory: SessionMemoryRepo,
    pub finance: crate::FinanceStorage,
    pub interaction_log: InteractionLogRepo,
    pub status_workflows: StatusWorkflowRepo,
    pub task_groups: TaskGroupRepo,
    pub custom_columns: CustomColumnRepo,
    pub entity_links: EntityLinkRepo,
    pub project_sources: ProjectSourceRepo,
    pub task_alarms: TaskAlarmsRepo,
    pub task_recurrence: TaskRecurrenceRepo,
    pub tasks: TaskRepo,
    pub tool_usage: ToolUsageRepo,
    pub dnd_override: DndOverrideRepo,
    pub reforge_state: ReforgeStateRepo,
    pub scheduled_fires: ScheduledFiresRepo,
    pub skill_version: SkillVersionRepo,
    pub notification_log: NotificationLogRepo,
    pub held_notifications: HeldNotificationsRepo,
    pub coding_approval_history: CodingApprovalHistoryRepo,
}

impl Repos {
    /// Create all repository instances from a connected pool.
    pub fn from_pool(pool: &crate::pool::StoragePool) -> Self {
        let db = pool.inner().clone();
        Self {
            agent_tasks: AgentTaskRepo::new(db.clone()),
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
            session_memory: SessionMemoryRepo::new(db.clone()),
            finance: crate::FinanceStorage::from_pool(&db),
            interaction_log: InteractionLogRepo::new(db.clone()),
            status_workflows: StatusWorkflowRepo::new(db.clone()),
            task_groups: TaskGroupRepo::new(db.clone()),
            custom_columns: CustomColumnRepo::new(db.clone()),
            entity_links: EntityLinkRepo::new(db.clone()),
            project_sources: ProjectSourceRepo::new(db.clone()),
            task_alarms: TaskAlarmsRepo::new(db.clone()),
            task_recurrence: TaskRecurrenceRepo::new(db.clone()),
            tasks: TaskRepo::new(db.clone()),
            tool_usage: ToolUsageRepo::new(db.clone()),
            dnd_override: DndOverrideRepo::new(db.clone()),
            reforge_state: ReforgeStateRepo::new(db.clone()),
            scheduled_fires: ScheduledFiresRepo::new(db.clone()),
            skill_version: SkillVersionRepo::new(db.clone()),
            notification_log: NotificationLogRepo::new(db.clone()),
            held_notifications: HeldNotificationsRepo::new(db.clone()),
            coding_approval_history: CodingApprovalHistoryRepo::new(pool.clone()),
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
    pub async fn cleanup_analytics(&self) -> Result<u64, crate::error::StorageError> {
        let now = jiff::Timestamp::now();

        let (a, b, c, d) = futures_util::try_join!(
            self.strategies.delete_older_than(90, now),
            self.outcomes.delete_older_than(30, now),
            self.interaction_log.delete_older_than(60, now),
            self.tool_usage.delete_older_than(90, now),
        )?;

        Ok(a + b + c + d)
    }
}
