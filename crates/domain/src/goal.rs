//! Goal domain types, errors, and SQL conversion helpers.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;
use uuid::Uuid;

// ── Error ──────────────────────────────────────────────────────────

/// Goal-specific errors
#[derive(Error, Debug)]
pub enum GoalError {
    #[error("Goal not found: {0}")]
    NotFound(String),

    #[error("Invalid goal state: {0}")]
    InvalidState(String),

    #[error("Goal store error: {0}")]
    StoreFailed(String),

    #[error("Goal validation failed: {0}")]
    ValidationFailed(String),
}

impl From<GoalError> for common::KlyntbotError {
    fn from(e: GoalError) -> Self {
        common::KlyntbotError::Goal(e.to_string())
    }
}

// ── Types ──────────────────────────────────────────────────────────

/// A strategic goal that spans multiple projects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub status: GoalStatus,
    /// Priority 1-5 (matches todo priority scale)
    pub priority: u8,
    pub target_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Number of plans that completed successfully for this goal.
    pub plans_completed: i32,
    /// Number of plans that failed for this goal.
    pub plans_failed: i32,
    /// Rolling average duration of completed plans in milliseconds.
    pub avg_duration_ms: Option<i64>,
    /// Timestamp of the most recent plan completion/failure.
    pub last_plan_at: Option<DateTime<Utc>>,
    /// Projects contributing to this goal
    pub linked_project_ids: Vec<Uuid>,
}

/// Current state of a goal.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GoalStatus {
    /// Currently pursuing
    #[default]
    Active,
    /// Temporarily suspended
    Paused,
    /// Completed successfully
    Achieved,
    /// No longer pursuing
    Abandoned,
}

impl GoalStatus {
    /// Validate if a transition from one status to another is allowed.
    ///
    /// Valid transitions:
    /// - Active -> Paused, Achieved, Abandoned
    /// - Paused -> Active, Abandoned
    /// - Achieved -> (final state, no transitions allowed)
    /// - Abandoned -> (final state, no transitions allowed)
    pub fn validate_transition(from: &GoalStatus, to: &GoalStatus) -> common::Result<()> {
        if from == to {
            return Ok(());
        }

        let valid = match from {
            GoalStatus::Active => matches!(
                to,
                GoalStatus::Paused | GoalStatus::Achieved | GoalStatus::Abandoned
            ),
            GoalStatus::Paused => matches!(to, GoalStatus::Active | GoalStatus::Abandoned),
            GoalStatus::Achieved | GoalStatus::Abandoned => false,
        };

        if valid {
            Ok(())
        } else {
            Err(
                GoalError::InvalidState(format!("Invalid state transition: {} → {}", from, to))
                    .into(),
            )
        }
    }
}

impl fmt::Display for GoalStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GoalStatus::Active => write!(f, "active"),
            GoalStatus::Paused => write!(f, "paused"),
            GoalStatus::Achieved => write!(f, "achieved"),
            GoalStatus::Abandoned => write!(f, "abandoned"),
        }
    }
}

impl FromStr for GoalStatus {
    type Err = common::KlyntbotError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(GoalStatus::Active),
            "paused" => Ok(GoalStatus::Paused),
            "achieved" => Ok(GoalStatus::Achieved),
            "abandoned" => Ok(GoalStatus::Abandoned),
            _ => Err(GoalError::ValidationFailed(format!(
                "unknown status '{}'. Valid: active, paused, achieved, abandoned",
                s
            ))
            .into()),
        }
    }
}

impl Goal {
    /// Validate that priority is within the allowed range (1-5).
    pub fn validate_priority(&self) -> common::Result<()> {
        if self.priority < 1 || self.priority > 5 {
            return Err(GoalError::ValidationFailed(format!(
                "priority must be 1-5, got {}",
                self.priority
            ))
            .into());
        }
        Ok(())
    }
}

/// Aggregated progress snapshot for a goal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalProgress {
    pub goal_id: Uuid,
    /// Overall completion (0.0-100.0)
    pub completion_percentage: f64,
    /// Human-readable summary, e.g. "3 of 5 plans succeeded"
    pub summary: String,
}

// ── Conversions ────────────────────────────────────────────────────

/// Convert a Goal domain type to a GoalRow for SQL persistence.
pub fn goal_to_row(goal: &Goal) -> storage::GoalRow {
    storage::GoalRow {
        id: goal.id,
        title: goal.title.clone(),
        description: goal.description.clone(),
        status: goal.status.to_string(),
        priority: goal.priority as i16,
        target_date: goal.target_date,
        created_at: goal.created_at,
        updated_at: goal.updated_at,
        plans_completed: goal.plans_completed,
        plans_failed: goal.plans_failed,
        avg_duration_ms: goal.avg_duration_ms,
        last_plan_at: goal.last_plan_at,
    }
}

/// Convert a GoalRow + linked project IDs back to a Goal domain type.
pub fn row_to_goal(row: storage::GoalRow, linked_project_ids: Vec<Uuid>) -> Goal {
    Goal {
        id: row.id,
        title: row.title,
        description: row.description,
        status: GoalStatus::from_str(&row.status).unwrap_or_default(),
        priority: row.priority as u8,
        target_date: row.target_date,
        created_at: row.created_at,
        updated_at: row.updated_at,
        plans_completed: row.plans_completed,
        plans_failed: row.plans_failed,
        avg_duration_ms: row.avg_duration_ms,
        last_plan_at: row.last_plan_at,
        linked_project_ids,
    }
}

/// Compute a GoalProgress from a Goal based on plan completion statistics.
pub fn compute_progress(goal: &Goal) -> GoalProgress {
    let total = goal.plans_completed + goal.plans_failed;
    let completion = if total == 0 {
        0.0
    } else {
        (goal.plans_completed as f64 / total as f64) * 100.0
    };

    let summary = if total == 0 {
        "No plans executed yet".to_string()
    } else {
        format!(
            "{:.0}% completion rate ({} of {} plans succeeded)",
            completion, goal.plans_completed, total
        )
    };

    GoalProgress {
        goal_id: goal.id,
        completion_percentage: completion,
        summary,
    }
}

/// Load a Goal from the repo by ID. Returns None if not found.
pub async fn load_goal(repo: &storage::GoalRepo, id: &Uuid) -> common::Result<Option<Goal>> {
    match repo.get(*id).await {
        Ok(row) => {
            let links = repo.get_project_links(*id).await?;
            let project_ids = links
                .into_iter()
                .filter_map(|l| Uuid::parse_str(&l.project_id).ok())
                .collect();
            Ok(Some(row_to_goal(row, project_ids)))
        }
        Err(storage::StorageError::NotFound(_)) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Save a Goal to the repo (create or update), including project links.
pub async fn save_goal(repo: &storage::GoalRepo, goal: &Goal) -> common::Result<()> {
    let row = goal_to_row(goal);
    // Try update first, fall back to create
    match repo.update(&row).await {
        Ok(_) => {}
        Err(storage::StorageError::NotFound(_)) => {
            repo.create(&row).await?;
        }
        Err(e) => return Err(e.into()),
    }

    // Sync project links: clear existing and re-link
    let existing = repo.get_project_links(goal.id).await?;
    for link in &existing {
        let _ = repo.unlink_project(goal.id, &link.project_id).await;
    }
    for pid in &goal.linked_project_ids {
        repo.link_project(goal.id, &pid.to_string()).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goal_status_valid_transitions() {
        // Active -> Paused, Achieved, Abandoned
        assert!(GoalStatus::validate_transition(&GoalStatus::Active, &GoalStatus::Paused).is_ok());
        assert!(
            GoalStatus::validate_transition(&GoalStatus::Active, &GoalStatus::Achieved).is_ok()
        );
        assert!(
            GoalStatus::validate_transition(&GoalStatus::Active, &GoalStatus::Abandoned).is_ok()
        );

        // Paused -> Active, Abandoned
        assert!(GoalStatus::validate_transition(&GoalStatus::Paused, &GoalStatus::Active).is_ok());
        assert!(
            GoalStatus::validate_transition(&GoalStatus::Paused, &GoalStatus::Abandoned).is_ok()
        );

        // No-op transitions are valid
        assert!(GoalStatus::validate_transition(&GoalStatus::Active, &GoalStatus::Active).is_ok());
        assert!(GoalStatus::validate_transition(&GoalStatus::Paused, &GoalStatus::Paused).is_ok());
        assert!(
            GoalStatus::validate_transition(&GoalStatus::Achieved, &GoalStatus::Achieved).is_ok()
        );
        assert!(
            GoalStatus::validate_transition(&GoalStatus::Abandoned, &GoalStatus::Abandoned).is_ok()
        );
    }

    #[test]
    fn test_goal_status_invalid_transitions() {
        // Paused -> Achieved (must go through Active first)
        assert!(
            GoalStatus::validate_transition(&GoalStatus::Paused, &GoalStatus::Achieved).is_err()
        );

        // Achieved -> anything (final state)
        assert!(
            GoalStatus::validate_transition(&GoalStatus::Achieved, &GoalStatus::Active).is_err()
        );
        assert!(
            GoalStatus::validate_transition(&GoalStatus::Achieved, &GoalStatus::Paused).is_err()
        );
        assert!(
            GoalStatus::validate_transition(&GoalStatus::Achieved, &GoalStatus::Abandoned).is_err()
        );

        // Abandoned -> anything (final state)
        assert!(
            GoalStatus::validate_transition(&GoalStatus::Abandoned, &GoalStatus::Active).is_err()
        );
        assert!(
            GoalStatus::validate_transition(&GoalStatus::Abandoned, &GoalStatus::Paused).is_err()
        );
        assert!(
            GoalStatus::validate_transition(&GoalStatus::Abandoned, &GoalStatus::Achieved).is_err()
        );
    }

    #[test]
    fn test_goal_progress() {
        let goal_id = Uuid::new_v4();
        let progress = GoalProgress {
            goal_id,
            completion_percentage: 45.0,
            summary: "45% completion rate (9 of 20 plans succeeded)".to_string(),
        };

        assert_eq!(progress.goal_id, goal_id);
        assert_eq!(progress.completion_percentage, 45.0);
        assert!(progress.summary.contains("45%"));
    }

    #[test]
    fn test_goal_serde_roundtrip() {
        let now = Utc::now();
        let goal = Goal {
            id: Uuid::new_v4(),
            title: "Ship v2".to_string(),
            description: "Release version 2.0".to_string(),
            status: GoalStatus::Active,
            priority: 1,
            target_date: None,
            created_at: now,
            updated_at: now,
            plans_completed: 3,
            plans_failed: 1,
            avg_duration_ms: Some(5000),
            last_plan_at: Some(now),
            linked_project_ids: vec![Uuid::new_v4()],
        };

        let json = serde_json::to_string(&goal).unwrap();
        let deserialized: Goal = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, goal.id);
        assert_eq!(deserialized.title, goal.title);
        assert!(matches!(deserialized.status, GoalStatus::Active));
        assert_eq!(deserialized.plans_completed, 3);
        assert_eq!(deserialized.plans_failed, 1);
        assert_eq!(deserialized.avg_duration_ms, Some(5000));
        assert!(deserialized.last_plan_at.is_some());
        assert_eq!(deserialized.linked_project_ids.len(), 1);
    }
}
