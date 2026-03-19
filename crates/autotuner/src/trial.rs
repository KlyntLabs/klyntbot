use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use common::TrialParams;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrialStatus {
    Pending,
    Active,
    Completed,
    Promoted,
    Reverted,
}

impl TrialStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Promoted => "promoted",
            Self::Reverted => "reverted",
        }
    }
}

impl fmt::Display for TrialStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for TrialStatus {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "active" => Self::Active,
            "completed" => Self::Completed,
            "promoted" => Self::Promoted,
            "reverted" => Self::Reverted,
            _ => Self::Pending,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trial {
    pub id: Uuid,
    pub experiment_id: Uuid,
    pub params: TrialParams,
    pub generation_reasoning: String,
    pub status: TrialStatus,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub result: Option<TrialResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrialResult {
    pub trial_id: Uuid,
    pub messages_scored: u32,
    pub correction_rate: f64,
    pub classification_accuracy: f64,
    pub avg_tokens_per_message: f64,
    pub avg_response_time_ms: f64,
    pub routing_stability: f64,
    pub memory_relevance: f64,
    pub user_satisfaction: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experiment {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub hypothesis: String,
    pub trend_analysis: String,
    pub recommendation_for_next: String,
    pub trial_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Champion {
    pub trial_id: Option<Uuid>,
    pub params: TrialParams,
    pub promoted_at: DateTime<Utc>,
    pub baseline_metrics: TrialResult,
    pub reason_for_promotion: String,
    pub impact_summary: String,
    pub consecutive_regression_days: u8,
}

impl Default for Champion {
    fn default() -> Self {
        Self {
            trial_id: None,
            params: TrialParams::default(),
            promoted_at: Utc::now(),
            baseline_metrics: TrialResult::default(),
            reason_for_promotion: "Using Config defaults".into(),
            impact_summary: "Baseline configuration".into(),
            consecutive_regression_days: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trial_status_roundtrip() {
        for status in [
            TrialStatus::Pending,
            TrialStatus::Active,
            TrialStatus::Completed,
            TrialStatus::Promoted,
            TrialStatus::Reverted,
        ] {
            let parsed: TrialStatus = status.as_str().parse().unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn champion_default_has_no_trial_id() {
        let c = Champion::default();
        assert!(c.trial_id.is_none());
        assert_eq!(c.reason_for_promotion, "Using Config defaults");
    }

    #[test]
    fn champion_serde_roundtrip() {
        let c = Champion::default();
        let json = serde_json::to_string(&c).unwrap();
        let back: Champion = serde_json::from_str(&json).unwrap();
        assert!(back.trial_id.is_none());
    }
}
