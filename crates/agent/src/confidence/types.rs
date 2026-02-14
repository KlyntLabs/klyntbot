//! Confidence assessment types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// End-to-end confidence assessment from LLM reasoning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceAssessment {
    /// Composite score 0.0–1.0.
    pub score: f32,
    /// What phase this was assessed at.
    pub phase: AssessmentPhase,
    /// LLM's reasoning for the score.
    pub reasoning: String,
    /// Specific dimensions that contributed.
    pub dimensions: ConfidenceDimensions,
    /// Recommended action based on score.
    #[serde(skip)]
    pub action: DecisionAction,
    /// Timestamp.
    pub assessed_at: DateTime<Utc>,
}

/// Assessment phase within the agent loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentPhase {
    PreTool,
    PostTool,
}

/// Per-dimension confidence breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceDimensions {
    /// How clear is the user's intent? (0.0–1.0)
    pub intent_clarity: f32,
    /// How well the chosen tool matches the need (0.0–1.0)
    pub tool_fit: f32,
    /// Whether enough information is available to execute (0.0–1.0)
    pub info_sufficiency: f32,
}

/// Action the agent should take based on confidence.
#[derive(Debug, Clone, Default)]
pub enum DecisionAction {
    /// Proceed with tool execution.
    #[default]
    Proceed,
    /// Ask user for clarification before proceeding.
    Clarify { questions: Vec<String> },
    /// Skip this tool call (not useful).
    Skip { reason: String },
}

/// Persistent log entry for decision tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionLogEntry {
    pub id: String,
    pub session_key: String,
    pub iteration: usize,
    pub tool_names: Vec<String>,
    pub user_message_preview: String,
    pub assessment: ConfidenceAssessment,
    pub outcome: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Raw JSON block the LLM emits inside `<confidence>` tags.
#[derive(Debug, Deserialize)]
pub(crate) struct RawConfidenceBlock {
    pub score: f32,
    pub intent_clarity: f32,
    pub tool_fit: f32,
    pub info_sufficiency: f32,
    pub reasoning: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_action_default() {
        assert!(matches!(DecisionAction::default(), DecisionAction::Proceed));
    }

    #[test]
    fn test_raw_block_deserialize() {
        let json = r#"{"score":0.85,"intent_clarity":0.9,"tool_fit":0.8,"info_sufficiency":0.85,"reasoning":"clear intent"}"#;
        let block: RawConfidenceBlock = serde_json::from_str(json).unwrap();
        assert!((block.score - 0.85).abs() < f32::EPSILON);
        assert_eq!(block.reasoning, "clear intent");
    }

    #[test]
    fn test_assessment_serialization() {
        let assessment = ConfidenceAssessment {
            score: 0.75,
            phase: AssessmentPhase::PreTool,
            reasoning: "test".to_string(),
            dimensions: ConfidenceDimensions {
                intent_clarity: 0.8,
                tool_fit: 0.7,
                info_sufficiency: 0.75,
            },
            action: DecisionAction::Proceed,
            assessed_at: Utc::now(),
        };

        let json = serde_json::to_string(&assessment).unwrap();
        let loaded: ConfidenceAssessment = serde_json::from_str(&json).unwrap();
        assert!((loaded.score - 0.75).abs() < f32::EPSILON);
        assert!(matches!(loaded.phase, AssessmentPhase::PreTool));
    }
}
