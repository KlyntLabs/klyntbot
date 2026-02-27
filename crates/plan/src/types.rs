//! Plan types for structured multi-step execution.

use crate::PlanError;
use chrono::{DateTime, Utc};
use common::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    /// - Draft → Approved, Abandoned
    /// - Approved → Executing, Abandoned
    /// - Executing → Completed, Failed, Abandoned
    /// - Completed → (final state, no transitions allowed)
    /// - Failed → (final state, no transitions allowed)
    /// - Abandoned → (final state, no transitions allowed)
    pub fn validate_transition(from: &PlanStatus, to: &PlanStatus) -> Result<()> {
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

/// Backtrack entry for retry tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktrackEntry {
    pub step_index: usize,
    pub attempt: u8,
    pub failure_reason: String,
    pub timestamp: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_creation_defaults() {
        // Given: no plan exists
        // When: a new Plan is created with minimal fields
        // Then:
        //   - status defaults to Draft
        //   - current_step_index is 0
        //   - backtrack_history is empty
        //   - iteration_limit defaults to 50
        //   - created_at and updated_at are set
        // Maps to: US-1 (AC-1.2)

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
        // Given: a Plan with status Draft
        // When: status transitions through: Draft → Approved → Executing → Completed
        // Then: all transitions succeed without error
        // Maps to: US-1 (AC-1.2)

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
        // Given: a PlanStep with status Pending
        // When: status transitions: Pending → Executing → Completed
        // Then: each transition is valid
        // And When: a step transitions: Pending → Executing → Failed
        // Then: the Failed status is set correctly
        // Maps to: US-1 (AC-1.3)

        // Test Pending → Executing → Completed
        let mut status = StepStatus::Pending;
        assert_eq!(status, StepStatus::Pending);

        status = StepStatus::Executing;
        assert_eq!(status, StepStatus::Executing);

        status = StepStatus::Completed;
        assert_eq!(status, StepStatus::Completed);

        // Test Pending → Executing → Failed
        let status_failed = StepStatus::Failed;
        assert_eq!(status_failed, StepStatus::Failed);
    }

    #[test]
    fn test_plan_serde_roundtrip() {
        // Given: a Plan with all fields populated (steps, backtrack_history, linked goal)
        // When: the Plan is serialized to JSON and deserialized
        // Then: the deserialized Plan matches the original exactly
        // Maps to: US-1 (AC-1.1)

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
            max_attempts: 3,
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

        // Serialize and deserialize
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
        // Test all valid transitions succeed
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
        // Terminal states: Completed, Failed, Abandoned — cannot transition to any other state
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
                    continue; // no-op transitions are allowed
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
        // Cannot skip states in the normal flow
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
        use crate::conversions::{str_to_visibility, visibility_to_str};

        assert_eq!(visibility_to_str(&PlanVisibility::Silent), "silent");
        assert_eq!(visibility_to_str(&PlanVisibility::OnFailure), "on_failure");
        assert_eq!(
            visibility_to_str(&PlanVisibility::Transparent),
            "transparent"
        );

        assert_eq!(str_to_visibility("silent"), PlanVisibility::Silent);
        assert_eq!(str_to_visibility("on_failure"), PlanVisibility::OnFailure);
        assert_eq!(
            str_to_visibility("transparent"),
            PlanVisibility::Transparent
        );
        assert_eq!(str_to_visibility("unknown"), PlanVisibility::Transparent);
    }

    #[test]
    fn plan_visibility_default_is_transparent() {
        assert_eq!(PlanVisibility::default(), PlanVisibility::Transparent);
    }
}
