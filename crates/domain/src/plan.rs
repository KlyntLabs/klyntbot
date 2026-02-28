//! Plan domain types, errors, and SQL conversion helpers.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;
use uuid::Uuid;

/// Default maximum attempts per plan step before the step is marked failed.
pub const DEFAULT_MAX_STEP_ATTEMPTS: u8 = 3;

// ── Error ──────────────────────────────────────────────────────────

/// Plan-specific errors
#[derive(Error, Debug)]
pub enum PlanError {
    #[error("Plan not found: {0}")]
    NotFound(String),

    #[error("Plan generation failed: {0}")]
    GenerationFailed(String),

    #[error("Invalid plan state: {0}")]
    InvalidState(String),

    #[error("Execution stalled at step {step_index}: {reason}")]
    ExecutionStalled { step_index: usize, reason: String },

    #[error("Backtrack limit reached at step {0}")]
    BacktrackLimitReached(usize),

    #[error("Plan store error: {0}")]
    StoreFailed(String),
}

impl From<PlanError> for common::KlyntbotError {
    fn from(e: PlanError) -> Self {
        common::KlyntbotError::Plan(e.to_string())
    }
}

// ── Types ──────────────────────────────────────────────────────────

/// Controls whether auto-generated plans appear in the UI.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PlanVisibility {
    /// Never shown. Auto-cleanup after 24h.
    Silent,
    /// Hidden until step failure, then surfaced for review.
    OnFailure,
    /// Always visible (user-created plans, current behavior).
    #[default]
    Transparent,
}

impl fmt::Display for PlanVisibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanVisibility::Silent => write!(f, "silent"),
            PlanVisibility::OnFailure => write!(f, "on_failure"),
            PlanVisibility::Transparent => write!(f, "transparent"),
        }
    }
}

impl FromStr for PlanVisibility {
    type Err = ();
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "silent" => Ok(PlanVisibility::Silent),
            "on_failure" => Ok(PlanVisibility::OnFailure),
            _ => Ok(PlanVisibility::Transparent),
        }
    }
}

/// A structured plan with multiple steps for sequential execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub id: Uuid,
    pub session_key: String,
    pub goal_id: Option<Uuid>,
    pub title: String,
    pub description: String,
    pub status: PlanStatus,
    pub steps: Vec<PlanStep>,
    pub current_step_index: usize,
    pub iteration_limit: usize,
    pub backtrack_history: Vec<BacktrackEntry>,
    #[serde(default)]
    pub visibility: PlanVisibility,
    #[serde(default)]
    pub task_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Plan status lifecycle
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanStatus {
    #[default]
    Draft,
    Approved,
    Executing,
    Completed,
    Failed,
    Abandoned,
}

impl PlanStatus {
    /// Validate if a transition from one status to another is allowed.
    ///
    /// Valid transitions:
    /// - Draft -> Approved, Abandoned
    /// - Approved -> Executing, Abandoned
    /// - Executing -> Completed, Failed, Abandoned
    /// - Completed -> (final state, no transitions allowed)
    /// - Failed -> (final state, no transitions allowed)
    /// - Abandoned -> (final state, no transitions allowed)
    pub fn validate_transition(from: &PlanStatus, to: &PlanStatus) -> common::Result<()> {
        // Allow no-op transitions (same state)
        if from == to {
            return Ok(());
        }

        let valid = match from {
            PlanStatus::Draft => matches!(to, PlanStatus::Approved | PlanStatus::Abandoned),
            PlanStatus::Approved => matches!(to, PlanStatus::Executing | PlanStatus::Abandoned),
            PlanStatus::Executing => matches!(
                to,
                PlanStatus::Completed | PlanStatus::Failed | PlanStatus::Abandoned
            ),
            // Final states - no transitions allowed
            PlanStatus::Completed | PlanStatus::Failed | PlanStatus::Abandoned => false,
        };

        if valid {
            Ok(())
        } else {
            Err(
                PlanError::InvalidState(format!("Invalid state transition: {:?} → {:?}", from, to))
                    .into(),
            )
        }
    }
}

impl fmt::Display for PlanStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanStatus::Draft => write!(f, "draft"),
            PlanStatus::Approved => write!(f, "approved"),
            PlanStatus::Executing => write!(f, "executing"),
            PlanStatus::Completed => write!(f, "completed"),
            PlanStatus::Failed => write!(f, "failed"),
            PlanStatus::Abandoned => write!(f, "abandoned"),
        }
    }
}

impl FromStr for PlanStatus {
    type Err = ();
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(PlanStatus::Draft),
            "approved" => Ok(PlanStatus::Approved),
            "executing" => Ok(PlanStatus::Executing),
            "completed" => Ok(PlanStatus::Completed),
            "failed" => Ok(PlanStatus::Failed),
            "abandoned" => Ok(PlanStatus::Abandoned),
            _ => Ok(PlanStatus::Draft),
        }
    }
}

/// A single step in a plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub id: Uuid,
    pub index: usize,
    pub description: String,
    pub reasoning: String,
    pub expected_tools: Vec<String>,
    pub status: StepStatus,
    pub attempt_count: u8,
    pub max_attempts: u8,
    pub result: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Step status lifecycle
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepStatus {
    #[default]
    Pending,
    Executing,
    Completed,
    Failed,
    Skipped,
}

impl fmt::Display for StepStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StepStatus::Pending => write!(f, "pending"),
            StepStatus::Executing => write!(f, "executing"),
            StepStatus::Completed => write!(f, "completed"),
            StepStatus::Failed => write!(f, "failed"),
            StepStatus::Skipped => write!(f, "skipped"),
        }
    }
}

impl FromStr for StepStatus {
    type Err = ();
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(StepStatus::Pending),
            "executing" => Ok(StepStatus::Executing),
            "completed" => Ok(StepStatus::Completed),
            "failed" => Ok(StepStatus::Failed),
            "skipped" => Ok(StepStatus::Skipped),
            _ => Ok(StepStatus::Pending),
        }
    }
}

/// Backtrack entry for retry tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktrackEntry {
    pub step_index: usize,
    pub attempt: u8,
    pub failure_reason: String,
    pub timestamp: DateTime<Utc>,
}

// ── Conversions ────────────────────────────────────────────────────

/// Convert a Plan domain type to a PlanRow for SQL persistence.
pub fn plan_to_row(plan: &Plan) -> common::Result<storage::PlanRow> {
    Ok(storage::PlanRow {
        id: plan.id,
        session_key: plan.session_key.clone(),
        goal_id: plan.goal_id,
        title: plan.title.clone(),
        description: plan.description.clone(),
        status: plan.status.to_string(),
        current_step_index: plan.current_step_index as i32,
        iteration_limit: plan.iteration_limit as i32,
        backtrack_history: serde_json::to_value(&plan.backtrack_history)
            .map_err(|e| common::KlyntbotError::Plan(format!("backtrack_history: {e}")))?,
        visibility: plan.visibility.to_string(),
        task_id: plan.task_id.clone(),
        created_at: plan.created_at,
        updated_at: plan.updated_at,
        completed_at: plan.completed_at,
    })
}

/// Convert a PlanStep to a PlanStepRow for SQL persistence.
pub fn step_to_row(step: &PlanStep, plan_id: Uuid) -> storage::PlanStepRow {
    storage::PlanStepRow {
        id: step.id,
        plan_id,
        step_index: step.index as i32,
        description: step.description.clone(),
        reasoning: step.reasoning.clone(),
        expected_tools: step.expected_tools.clone(),
        status: step.status.to_string(),
        attempt_count: step.attempt_count as i16,
        max_attempts: step.max_attempts as i16,
        result: step.result.clone(),
        started_at: step.started_at,
        completed_at: step.completed_at,
    }
}

/// Convert a PlanRow + PlanStepRows back to a Plan domain type.
pub fn row_to_plan(row: storage::PlanRow, step_rows: Vec<storage::PlanStepRow>) -> Plan {
    let steps: Vec<PlanStep> = step_rows
        .into_iter()
        .map(|sr| PlanStep {
            id: sr.id,
            index: sr.step_index as usize,
            description: sr.description,
            reasoning: sr.reasoning,
            expected_tools: sr.expected_tools,
            status: sr.status.parse().unwrap_or_default(),
            attempt_count: sr.attempt_count as u8,
            max_attempts: sr.max_attempts as u8,
            result: sr.result,
            started_at: sr.started_at,
            completed_at: sr.completed_at,
        })
        .collect();

    Plan {
        id: row.id,
        session_key: row.session_key,
        goal_id: row.goal_id,
        title: row.title,
        description: row.description,
        status: row.status.parse().unwrap_or_default(),
        steps,
        current_step_index: row.current_step_index as usize,
        iteration_limit: row.iteration_limit as usize,
        backtrack_history: serde_json::from_value::<Vec<BacktrackEntry>>(row.backtrack_history)
            .unwrap_or_default(),
        visibility: row.visibility.parse().unwrap_or_default(),
        task_id: row.task_id,
        created_at: row.created_at,
        updated_at: row.updated_at,
        completed_at: row.completed_at,
    }
}

/// Load a Plan from the repo by ID (including its steps). Returns None if not found.
pub async fn load_plan(repo: &storage::PlanRepo, id: &Uuid) -> common::Result<Option<Plan>> {
    match repo.get(*id).await {
        Ok(row) => {
            let steps = repo.get_steps(*id).await?;
            Ok(Some(row_to_plan(row, steps)))
        }
        Err(storage::StorageError::NotFound(_)) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Save (upsert) a Plan + all its steps to the repo.
pub async fn save_plan(repo: &storage::PlanRepo, plan: &Plan) -> common::Result<()> {
    let row = plan_to_row(plan)?;
    repo.upsert(&row).await?;

    for step in &plan.steps {
        let step_row = step_to_row(step, plan.id);
        repo.upsert_step(&step_row).await?;
    }

    Ok(())
}

/// Get the most recent active plan for a session.
/// Returns Draft, Approved, or Executing plans only.
pub async fn get_active_plan(
    repo: &storage::PlanRepo,
    session_key: &str,
) -> common::Result<Option<Plan>> {
    let Some(row) = repo.get_active(session_key).await? else {
        return Ok(None);
    };
    let steps = repo.get_steps(row.id).await?;
    Ok(Some(row_to_plan(row, steps)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_creation_defaults() {
        let now = Utc::now();
        let plan = Plan {
            id: Uuid::new_v4(),
            session_key: "test-session".to_string(),
            goal_id: None,
            title: "Test Plan".to_string(),
            description: "A test plan".to_string(),
            status: PlanStatus::default(),
            steps: vec![],
            current_step_index: 0,
            iteration_limit: 50,
            backtrack_history: vec![],
            visibility: PlanVisibility::default(),
            task_id: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
        };

        assert_eq!(plan.status, PlanStatus::Draft);
        assert_eq!(plan.current_step_index, 0);
        assert!(plan.backtrack_history.is_empty());
        assert_eq!(plan.iteration_limit, 50);
        assert!(plan.created_at <= Utc::now());
        assert!(plan.updated_at <= Utc::now());
    }

    #[test]
    fn test_plan_status_transitions_valid() {
        let mut status = PlanStatus::Draft;
        assert_eq!(status, PlanStatus::Draft);

        status = PlanStatus::Approved;
        assert_eq!(status, PlanStatus::Approved);

        status = PlanStatus::Executing;
        assert_eq!(status, PlanStatus::Executing);

        status = PlanStatus::Completed;
        assert_eq!(status, PlanStatus::Completed);
    }

    #[test]
    fn test_step_status_transitions() {
        let mut status = StepStatus::Pending;
        assert_eq!(status, StepStatus::Pending);

        status = StepStatus::Executing;
        assert_eq!(status, StepStatus::Executing);

        status = StepStatus::Completed;
        assert_eq!(status, StepStatus::Completed);

        let status_failed = StepStatus::Failed;
        assert_eq!(status_failed, StepStatus::Failed);
    }

    #[test]
    fn test_plan_serde_roundtrip() {
        let now = Utc::now();
        let goal_id = Uuid::new_v4();
        let step = PlanStep {
            id: Uuid::new_v4(),
            index: 0,
            description: "Test step".to_string(),
            reasoning: "Test reasoning".to_string(),
            expected_tools: vec!["test_tool".to_string()],
            status: StepStatus::Pending,
            attempt_count: 0,
            max_attempts: DEFAULT_MAX_STEP_ATTEMPTS,
            result: None,
            started_at: None,
            completed_at: None,
        };

        let backtrack = BacktrackEntry {
            step_index: 0,
            attempt: 1,
            failure_reason: "Test failure".to_string(),
            timestamp: now,
        };

        let plan = Plan {
            id: Uuid::new_v4(),
            session_key: "session-1".to_string(),
            goal_id: Some(goal_id),
            title: "Test Plan".to_string(),
            description: "A comprehensive test plan".to_string(),
            status: PlanStatus::Draft,
            steps: vec![step],
            current_step_index: 0,
            iteration_limit: 50,
            backtrack_history: vec![backtrack],
            visibility: PlanVisibility::default(),
            task_id: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
        };

        let json = serde_json::to_string(&plan).unwrap();
        let deserialized: Plan = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, plan.id);
        assert_eq!(deserialized.session_key, plan.session_key);
        assert_eq!(deserialized.goal_id, Some(goal_id));
        assert_eq!(deserialized.title, plan.title);
        assert_eq!(deserialized.description, plan.description);
        assert_eq!(deserialized.status, PlanStatus::Draft);
        assert_eq!(deserialized.steps.len(), 1);
        assert_eq!(deserialized.steps[0].description, "Test step");
        assert_eq!(deserialized.backtrack_history.len(), 1);
        assert_eq!(deserialized.backtrack_history[0].step_index, 0);
        assert_eq!(deserialized.current_step_index, 0);
        assert_eq!(deserialized.iteration_limit, 50);
    }

    #[test]
    fn test_valid_status_transitions() {
        assert!(PlanStatus::validate_transition(&PlanStatus::Draft, &PlanStatus::Approved).is_ok());
        assert!(
            PlanStatus::validate_transition(&PlanStatus::Draft, &PlanStatus::Abandoned).is_ok()
        );
        assert!(
            PlanStatus::validate_transition(&PlanStatus::Approved, &PlanStatus::Executing).is_ok()
        );
        assert!(
            PlanStatus::validate_transition(&PlanStatus::Approved, &PlanStatus::Abandoned).is_ok()
        );
        assert!(
            PlanStatus::validate_transition(&PlanStatus::Executing, &PlanStatus::Completed).is_ok()
        );
        assert!(
            PlanStatus::validate_transition(&PlanStatus::Executing, &PlanStatus::Failed).is_ok()
        );
        assert!(
            PlanStatus::validate_transition(&PlanStatus::Executing, &PlanStatus::Abandoned).is_ok()
        );

        // No-op transitions (same state) should succeed
        assert!(PlanStatus::validate_transition(&PlanStatus::Draft, &PlanStatus::Draft).is_ok());
        assert!(
            PlanStatus::validate_transition(&PlanStatus::Approved, &PlanStatus::Approved).is_ok()
        );
    }

    #[test]
    fn test_invalid_status_transitions_from_terminal_states() {
        let terminal = [
            PlanStatus::Completed,
            PlanStatus::Failed,
            PlanStatus::Abandoned,
        ];
        let all = [
            PlanStatus::Draft,
            PlanStatus::Approved,
            PlanStatus::Executing,
            PlanStatus::Completed,
            PlanStatus::Failed,
            PlanStatus::Abandoned,
        ];

        for from in &terminal {
            for to in &all {
                if from == to {
                    continue;
                }
                assert!(
                    PlanStatus::validate_transition(from, to).is_err(),
                    "Expected error for {:?} → {:?}",
                    from,
                    to
                );
            }
        }
    }

    #[test]
    fn test_invalid_status_transitions_skipping_states() {
        assert!(
            PlanStatus::validate_transition(&PlanStatus::Draft, &PlanStatus::Executing).is_err()
        );
        assert!(
            PlanStatus::validate_transition(&PlanStatus::Draft, &PlanStatus::Completed).is_err()
        );
        assert!(PlanStatus::validate_transition(&PlanStatus::Draft, &PlanStatus::Failed).is_err());
        assert!(
            PlanStatus::validate_transition(&PlanStatus::Approved, &PlanStatus::Completed).is_err()
        );
        assert!(
            PlanStatus::validate_transition(&PlanStatus::Approved, &PlanStatus::Failed).is_err()
        );
    }

    #[test]
    fn plan_visibility_roundtrip() {
        assert_eq!(PlanVisibility::Silent.to_string(), "silent");
        assert_eq!(PlanVisibility::OnFailure.to_string(), "on_failure");
        assert_eq!(PlanVisibility::Transparent.to_string(), "transparent");

        assert_eq!(
            "silent".parse::<PlanVisibility>().unwrap(),
            PlanVisibility::Silent
        );
        assert_eq!(
            "on_failure".parse::<PlanVisibility>().unwrap(),
            PlanVisibility::OnFailure
        );
        assert_eq!(
            "transparent".parse::<PlanVisibility>().unwrap(),
            PlanVisibility::Transparent
        );
        assert_eq!(
            "unknown".parse::<PlanVisibility>().unwrap(),
            PlanVisibility::Transparent
        );
    }

    #[test]
    fn plan_visibility_default_is_transparent() {
        assert_eq!(PlanVisibility::default(), PlanVisibility::Transparent);
    }
}
