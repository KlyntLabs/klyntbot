//! KeyResult domain type — measurable result within an objective.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyResult {
    pub id: String,
    pub objective_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: KeyResultStatus,
    pub tracking_mode: TrackingMode,
    pub target_value: Option<f64>,
    pub current_value: f64,
    pub unit: Option<String>,
    pub progress: f64, // 0.0-100.0
    pub due_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl KeyResult {
    pub fn generate_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    /// Recalculate progress based on tracking mode.
    /// For metric mode: current_value / target_value * 100 (clamped 0-100).
    /// For action mode: caller must provide completed/total counts.
    pub fn recalculate_metric_progress(&mut self) {
        if self.tracking_mode == TrackingMode::Metric {
            self.progress = match self.target_value {
                Some(target) if target > 0.0 => {
                    (self.current_value / target * 100.0).clamp(0.0, 100.0)
                }
                _ => 0.0,
            };
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TrackingMode {
    Metric,
    #[default]
    Action,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum KeyResultStatus {
    Active,
    Completed,
    Abandoned,
}

impl KeyResultStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Abandoned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_progress_calculation() {
        let mut kr = KeyResult {
            id: "test1234".into(),
            objective_id: "obj12345".into(),
            title: "Test KR".into(),
            description: None,
            status: KeyResultStatus::Active,
            tracking_mode: TrackingMode::Metric,
            target_value: Some(100.0),
            current_value: 60.0,
            unit: Some("%".into()),
            progress: 0.0,
            due_date: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            completed_at: None,
        };
        kr.recalculate_metric_progress();
        assert!((kr.progress - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metric_progress_clamped() {
        let mut kr = KeyResult {
            id: "test1234".into(),
            objective_id: "obj12345".into(),
            title: "Test".into(),
            description: None,
            status: KeyResultStatus::Active,
            tracking_mode: TrackingMode::Metric,
            target_value: Some(50.0),
            current_value: 100.0,
            unit: None,
            progress: 0.0,
            due_date: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            completed_at: None,
        };
        kr.recalculate_metric_progress();
        assert!((kr.progress - 100.0).abs() < f64::EPSILON);
    }
}
