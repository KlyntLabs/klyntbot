//! Goal domain types for the goal engine.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

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
    /// Progress indicators
    pub metrics: Vec<Metric>,
    /// Projects contributing to this goal
    pub linked_project_ids: Vec<Uuid>,
    /// Extensible key-value metadata
    pub metadata: HashMap<String, String>,
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
    /// - Active → Paused, Achieved, Abandoned
    /// - Paused → Active, Abandoned
    /// - Achieved → (final state, no transitions allowed)
    /// - Abandoned → (final state, no transitions allowed)
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
            Err(crate::GoalError::InvalidState(format!(
                "Invalid state transition: {} → {}",
                from, to
            ))
            .into())
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
            _ => Err(crate::GoalError::ValidationFailed(format!(
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
            return Err(crate::GoalError::ValidationFailed(format!(
                "priority must be 1-5, got {}",
                self.priority
            ))
            .into());
        }
        Ok(())
    }
}

/// A quantifiable progress indicator for a goal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub current: f64,
    pub target: f64,
    pub unit: String,
}

impl Metric {
    /// Calculate progress as a percentage (0.0-100.0), capped at 100.
    pub fn progress_percentage(&self) -> f64 {
        if self.target == 0.0 {
            return 0.0;
        }
        ((self.current / self.target) * 100.0).min(100.0)
    }
}

/// Aggregated progress snapshot for a goal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalProgress {
    pub goal_id: Uuid,
    /// Overall completion (0.0-100.0)
    pub completion_percentage: f64,
    pub metrics: Vec<Metric>,
    /// Human-readable summary, e.g. "3 of 5 projects completed"
    pub summary: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goal_status_valid_transitions() {
        // Active → Paused, Achieved, Abandoned
        assert!(GoalStatus::validate_transition(&GoalStatus::Active, &GoalStatus::Paused).is_ok());
        assert!(
            GoalStatus::validate_transition(&GoalStatus::Active, &GoalStatus::Achieved).is_ok()
        );
        assert!(
            GoalStatus::validate_transition(&GoalStatus::Active, &GoalStatus::Abandoned).is_ok()
        );

        // Paused → Active, Abandoned
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
        // Paused → Achieved (must go through Active first)
        assert!(
            GoalStatus::validate_transition(&GoalStatus::Paused, &GoalStatus::Achieved).is_err()
        );

        // Achieved → anything (final state)
        assert!(
            GoalStatus::validate_transition(&GoalStatus::Achieved, &GoalStatus::Active).is_err()
        );
        assert!(
            GoalStatus::validate_transition(&GoalStatus::Achieved, &GoalStatus::Paused).is_err()
        );
        assert!(
            GoalStatus::validate_transition(&GoalStatus::Achieved, &GoalStatus::Abandoned).is_err()
        );

        // Abandoned → anything (final state)
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
    fn test_metric_progress_calculation() {
        let metric = Metric {
            name: "Tasks completed".to_string(),
            current: 3.0,
            target: 10.0,
            unit: "tasks".to_string(),
        };

        assert_eq!(metric.name, "Tasks completed");
        assert_eq!(metric.current, 3.0);
        assert_eq!(metric.target, 10.0);
        let progress = metric.progress_percentage();
        assert!((progress - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metric_progress_zero_target() {
        let metric = Metric {
            name: "Revenue".to_string(),
            current: 100.0,
            target: 0.0,
            unit: "$".to_string(),
        };

        // Should return 0.0 for zero target to avoid division by zero
        assert_eq!(metric.progress_percentage(), 0.0);
    }

    #[test]
    fn test_metric_progress_capped_at_100() {
        let metric = Metric {
            name: "Steps".to_string(),
            current: 15000.0,
            target: 10000.0,
            unit: "steps".to_string(),
        };

        assert_eq!(metric.progress_percentage(), 100.0);
    }

    #[test]
    fn test_goal_progress() {
        let goal_id = Uuid::new_v4();
        let progress = GoalProgress {
            goal_id,
            completion_percentage: 45.0,
            metrics: vec![Metric {
                name: "Projects done".to_string(),
                current: 2.0,
                target: 5.0,
                unit: "projects".to_string(),
            }],
            summary: "2 of 5 projects completed".to_string(),
        };

        assert_eq!(progress.goal_id, goal_id);
        assert_eq!(progress.completion_percentage, 45.0);
        assert_eq!(progress.metrics.len(), 1);
        assert_eq!(progress.summary, "2 of 5 projects completed");
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
            metrics: vec![Metric {
                name: "Features".to_string(),
                current: 5.0,
                target: 10.0,
                unit: "features".to_string(),
            }],
            linked_project_ids: vec![Uuid::new_v4()],
            metadata: HashMap::from([("category".to_string(), "engineering".to_string())]),
        };

        let json = serde_json::to_string(&goal).unwrap();
        let deserialized: Goal = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, goal.id);
        assert_eq!(deserialized.title, goal.title);
        assert!(matches!(deserialized.status, GoalStatus::Active));
        assert_eq!(deserialized.metrics.len(), 1);
        assert_eq!(deserialized.linked_project_ids.len(), 1);
        assert_eq!(
            deserialized.metadata.get("category").unwrap(),
            "engineering"
        );
    }
}
