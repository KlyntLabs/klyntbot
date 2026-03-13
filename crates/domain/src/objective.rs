//! Objective domain type — OKR objective within a project.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Objective {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: ObjectiveStatus,
    pub priority: Option<u8>,
    pub due_date: Option<DateTime<Utc>>,
    pub progress: f64, // 0.0-100.0, aggregated from KRs
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl Objective {
    pub fn generate_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ObjectiveStatus {
    Active,
    Paused,
    Completed,
    Abandoned,
}

impl ObjectiveStatus {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "active" => Some(Self::Active),
            "paused" => Some(Self::Paused),
            "completed" => Some(Self::Completed),
            "abandoned" => Some(Self::Abandoned),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Abandoned => "abandoned",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Abandoned)
    }
}

impl std::fmt::Display for ObjectiveStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_objective_status_roundtrip() {
        for status in [
            ObjectiveStatus::Active,
            ObjectiveStatus::Paused,
            ObjectiveStatus::Completed,
            ObjectiveStatus::Abandoned,
        ] {
            let s = status.as_str();
            assert_eq!(ObjectiveStatus::from_str_loose(s), Some(status));
        }
    }

    #[test]
    fn test_terminal_states() {
        assert!(!ObjectiveStatus::Active.is_terminal());
        assert!(!ObjectiveStatus::Paused.is_terminal());
        assert!(ObjectiveStatus::Completed.is_terminal());
        assert!(ObjectiveStatus::Abandoned.is_terminal());
    }
}
