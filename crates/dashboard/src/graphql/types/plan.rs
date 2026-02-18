//! GraphQL types wrapping plan::Plan and related types.

use async_graphql::{Enum, InputObject, Object, SimpleObject, ID};
use chrono::{DateTime, Utc};
use plan::{
    BacktrackEntry as DomainBacktrackEntry, Plan as DomainPlan, PlanStatus as DomainPlanStatus,
    PlanStep as DomainPlanStep, StepStatus as DomainStepStatus,
};

// ── PlanStatus ────────────────────────────────────────────────────────────────

#[derive(Enum, Clone, Copy, PartialEq, Eq)]
pub enum GqlPlanStatus {
    Draft,
    Approved,
    Executing,
    Completed,
    Failed,
    Abandoned,
}

impl From<DomainPlanStatus> for GqlPlanStatus {
    fn from(s: DomainPlanStatus) -> Self {
        match s {
            DomainPlanStatus::Draft => GqlPlanStatus::Draft,
            DomainPlanStatus::Approved => GqlPlanStatus::Approved,
            DomainPlanStatus::Executing => GqlPlanStatus::Executing,
            DomainPlanStatus::Completed => GqlPlanStatus::Completed,
            DomainPlanStatus::Failed => GqlPlanStatus::Failed,
            DomainPlanStatus::Abandoned => GqlPlanStatus::Abandoned,
        }
    }
}

impl From<GqlPlanStatus> for DomainPlanStatus {
    fn from(s: GqlPlanStatus) -> Self {
        match s {
            GqlPlanStatus::Draft => DomainPlanStatus::Draft,
            GqlPlanStatus::Approved => DomainPlanStatus::Approved,
            GqlPlanStatus::Executing => DomainPlanStatus::Executing,
            GqlPlanStatus::Completed => DomainPlanStatus::Completed,
            GqlPlanStatus::Failed => DomainPlanStatus::Failed,
            GqlPlanStatus::Abandoned => DomainPlanStatus::Abandoned,
        }
    }
}

// ── StepStatus ────────────────────────────────────────────────────────────────

#[derive(Enum, Clone, Copy, PartialEq, Eq)]
pub enum GqlStepStatus {
    Pending,
    Executing,
    Completed,
    Failed,
    Skipped,
}

impl From<DomainStepStatus> for GqlStepStatus {
    fn from(s: DomainStepStatus) -> Self {
        match s {
            DomainStepStatus::Pending => GqlStepStatus::Pending,
            DomainStepStatus::Executing => GqlStepStatus::Executing,
            DomainStepStatus::Completed => GqlStepStatus::Completed,
            DomainStepStatus::Failed => GqlStepStatus::Failed,
            DomainStepStatus::Skipped => GqlStepStatus::Skipped,
        }
    }
}

// ── PlanStep ──────────────────────────────────────────────────────────────────

/// A single step in a plan.
#[derive(SimpleObject, Clone)]
pub struct GqlPlanStep {
    pub id: ID,
    pub index: i32,
    pub description: String,
    pub reasoning: String,
    pub expected_tools: Vec<String>,
    pub status: GqlStepStatus,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub result: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl From<DomainPlanStep> for GqlPlanStep {
    fn from(s: DomainPlanStep) -> Self {
        Self {
            id: s.id.to_string().into(),
            index: s.index as i32,
            description: s.description,
            reasoning: s.reasoning,
            expected_tools: s.expected_tools,
            status: s.status.into(),
            attempt_count: s.attempt_count as i32,
            max_attempts: s.max_attempts as i32,
            result: s.result,
            started_at: s.started_at,
            completed_at: s.completed_at,
        }
    }
}

// ── BacktrackEntry ────────────────────────────────────────────────────────────

/// A recorded backtrack event during plan execution.
#[derive(SimpleObject, Clone)]
pub struct GqlBacktrackEntry {
    pub step_index: i32,
    pub attempt: i32,
    pub failure_reason: String,
    pub timestamp: DateTime<Utc>,
}

impl From<DomainBacktrackEntry> for GqlBacktrackEntry {
    fn from(e: DomainBacktrackEntry) -> Self {
        Self {
            step_index: e.step_index as i32,
            attempt: e.attempt as i32,
            failure_reason: e.failure_reason,
            timestamp: e.timestamp,
        }
    }
}

// ── GqlPlan ───────────────────────────────────────────────────────────────────

/// A structured plan with multiple steps.
pub struct GqlPlan(pub DomainPlan);

#[Object]
impl GqlPlan {
    async fn id(&self) -> ID {
        self.0.id.to_string().into()
    }

    async fn session_key(&self) -> &str {
        &self.0.session_key
    }

    async fn goal_id(&self) -> Option<String> {
        self.0.goal_id.map(|id| id.to_string())
    }

    async fn title(&self) -> &str {
        &self.0.title
    }

    async fn description(&self) -> &str {
        &self.0.description
    }

    async fn status(&self) -> GqlPlanStatus {
        self.0.status.clone().into()
    }

    async fn steps(&self) -> Vec<GqlPlanStep> {
        self.0
            .steps
            .iter()
            .cloned()
            .map(GqlPlanStep::from)
            .collect()
    }

    async fn current_step_index(&self) -> i32 {
        self.0.current_step_index as i32
    }

    async fn iteration_limit(&self) -> i32 {
        self.0.iteration_limit as i32
    }

    async fn backtrack_history(&self) -> Vec<GqlBacktrackEntry> {
        self.0
            .backtrack_history
            .iter()
            .cloned()
            .map(GqlBacktrackEntry::from)
            .collect()
    }

    async fn created_at(&self) -> DateTime<Utc> {
        self.0.created_at
    }

    async fn updated_at(&self) -> DateTime<Utc> {
        self.0.updated_at
    }

    async fn completed_at(&self) -> Option<DateTime<Utc>> {
        self.0.completed_at
    }
}

// ── Input types ───────────────────────────────────────────────────────────────

/// Input for creating a step within a new plan.
#[derive(InputObject)]
pub struct CreatePlanStepInput {
    pub description: String,
    pub reasoning: Option<String>,
    pub expected_tools: Option<Vec<String>>,
}

/// Input for creating a new plan.
#[derive(InputObject)]
pub struct CreatePlanInput {
    pub title: String,
    pub description: Option<String>,
    pub session_key: Option<String>,
    pub steps: Option<Vec<CreatePlanStepInput>>,
    pub goal_id: Option<String>,
}

/// Filter for listing plans.
#[derive(InputObject, Default)]
pub struct PlanFilter {
    pub status: Option<GqlPlanStatus>,
    pub session_key: Option<String>,
}
