//! Goal domain types for the goal engine.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    fn test_goal_construction() {
        let now = Utc::now();
        let goal = Goal {
            id: Uuid::new_v4(),
            title: "Learn Rust".to_string(),
            description: "Master Rust programming language".to_string(),
            status: GoalStatus::Active,
            priority: 2,
            target_date: Some(now + chrono::Duration::days(90)),
            created_at: now,
            updated_at: now,
            metrics: vec![],
            linked_project_ids: vec![],
            metadata: HashMap::new(),
        };

        assert_eq!(goal.title, "Learn Rust");
        assert_eq!(goal.priority, 2);
        assert!(matches!(goal.status, GoalStatus::Active));
        assert!(goal.target_date.is_some());
    }

    #[test]
    fn test_goal_status_transitions() {
        // Active can transition to any state
        let statuses = [
            GoalStatus::Active,
            GoalStatus::Paused,
            GoalStatus::Achieved,
            GoalStatus::Abandoned,
        ];

        // Verify all variants are distinct via display/debug
        let debug_strings: Vec<String> = statuses.iter().map(|s| format!("{:?}", s)).collect();
        assert_eq!(debug_strings.len(), 4);
        assert!(debug_strings.contains(&"Active".to_string()));
        assert!(debug_strings.contains(&"Paused".to_string()));
        assert!(debug_strings.contains(&"Achieved".to_string()));
        assert!(debug_strings.contains(&"Abandoned".to_string()));
    }

    #[test]
    fn test_goal_status_default() {
        let status = GoalStatus::default();
        assert!(matches!(status, GoalStatus::Active));
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
            metrics: vec![
                Metric {
                    name: "Projects done".to_string(),
                    current: 2.0,
                    target: 5.0,
                    unit: "projects".to_string(),
                },
            ],
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
        assert_eq!(deserialized.metadata.get("category").unwrap(), "engineering");
    }

    #[test]
    fn test_goal_status_serde() {
        let statuses = vec![
            (GoalStatus::Active, "\"Active\""),
            (GoalStatus::Paused, "\"Paused\""),
            (GoalStatus::Achieved, "\"Achieved\""),
            (GoalStatus::Abandoned, "\"Abandoned\""),
        ];

        for (status, expected_json) in statuses {
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, expected_json);
            let deserialized: GoalStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(format!("{:?}", deserialized), format!("{:?}", status));
        }
    }
}
