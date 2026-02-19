//! Core data types for the learning system.
//!
//! All types follow privacy-by-omission: `OutcomeRecord` does NOT contain
//! tool arguments or user messages. Only tool name, success, duration,
//! confidence score, and hashed session key are persisted.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::confidence::types::ConfidenceDimensions;

/// A single tool execution outcome record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeRecord {
    pub id: String,
    /// Channel prefix + hashed suffix (e.g., "telegram:a1b2c3").
    pub session_key: String,
    pub tool_name: String,
    // Privacy-by-omission: no tool_args or user_message fields.
    pub success: bool,
    /// Categorized error type (e.g., "validation", "timeout", "permission").
    pub error_category: Option<String>,
    pub duration_ms: u64,
    /// Confidence score from the pre-tool assessment, if any.
    pub confidence_score: Option<f32>,
    /// Full dimension breakdown, if available.
    pub confidence_dimensions: Option<ConfidenceDimensions>,
    pub execution_mode: ExecutionMode,
    pub created_at: DateTime<Utc>,
}

/// Whether the tool was called during chat or plan execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    Chat,
    PlanStep { plan_id: String, step_index: usize },
}

/// Analysis results computed by LearningAnalyzer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub computed_at: DateTime<Utc>,
    pub total_outcomes: usize,
    pub per_tool_stats: HashMap<String, ToolStats>,
    pub suggested_threshold: f32,
    /// How confident the analyzer is in the suggestion (0.0–1.0).
    pub threshold_confidence: f32,
    pub enrichment_stats: EnrichmentStats,
}

/// Per-tool aggregate statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStats {
    pub total_calls: usize,
    pub success_count: usize,
    pub avg_duration_ms: f64,
    /// Success rates bucketed by confidence score ranges.
    pub success_rate_by_confidence_band: Vec<ConfidenceBand>,
}

/// A confidence range bucket with success statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceBand {
    pub lower: f32,
    pub upper: f32,
    pub total: usize,
    pub successes: usize,
    pub success_rate: f32,
}

/// Aggregate enrichment acceptance statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnrichmentStats {
    pub total_suggestions: usize,
    pub accepted_count: usize,
    pub overridden_count: usize,
    pub acceptance_rate: f32,
    pub per_field: HashMap<String, FieldAcceptanceStats>,
}

/// Per-field enrichment acceptance breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldAcceptanceStats {
    pub total: usize,
    pub accepted: usize,
    pub acceptance_rate: f32,
}

// Re-export EnrichmentFeedbackEntry from the tools crate (canonical definition).
pub use tools::learning_feedback::EnrichmentFeedbackEntry;

/// Persistent state for adaptive threshold adjustment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveThresholdState {
    pub current_threshold: f32,
    pub last_analysis: Option<AnalysisResult>,
    pub threshold_history: Vec<ThresholdChange>,
    pub updated_at: DateTime<Utc>,
}

/// A single threshold change event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdChange {
    pub from: f32,
    pub to: f32,
    pub reason: String,
    pub timestamp: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_learning_types_serde_roundtrip() {
        // OutcomeRecord roundtrip
        let record = OutcomeRecord {
            id: "test-001".to_string(),
            session_key: "telegram:a1b2c3".to_string(),
            tool_name: "todo".to_string(),
            success: true,
            error_category: None,
            duration_ms: 42,
            confidence_score: Some(0.85),
            confidence_dimensions: Some(ConfidenceDimensions {
                intent_clarity: 0.9,
                tool_fit: 0.8,
                info_sufficiency: 0.85,
            }),
            execution_mode: ExecutionMode::Chat,
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&record).unwrap();
        let loaded: OutcomeRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.id, "test-001", "failed for: OutcomeRecord.id");
        assert_eq!(loaded.tool_name, "todo", "failed for: OutcomeRecord.tool_name");
        assert!((loaded.confidence_score.unwrap() - 0.85).abs() < f32::EPSILON);

        // ExecutionMode::PlanStep roundtrip
        let mode = ExecutionMode::PlanStep {
            plan_id: "plan-abc".to_string(),
            step_index: 3,
        };
        let json = serde_json::to_string(&mode).unwrap();
        let loaded: ExecutionMode = serde_json::from_str(&json).unwrap();
        match loaded {
            ExecutionMode::PlanStep { plan_id, step_index } => {
                assert_eq!(plan_id, "plan-abc", "failed for: PlanStep.plan_id");
                assert_eq!(step_index, 3, "failed for: PlanStep.step_index");
            }
            _ => panic!("Expected PlanStep variant"),
        }

        // EnrichmentFeedbackEntry roundtrip
        let entry = EnrichmentFeedbackEntry {
            task_id: "todo-123".to_string(),
            field: "priority".to_string(),
            suggested_value: "1".to_string(),
            actual_value: Some("2".to_string()),
            accepted: false,
            confidence: 0.75,
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let loaded: EnrichmentFeedbackEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.task_id, "todo-123", "failed for: EnrichmentFeedbackEntry.task_id");
        assert!(!loaded.accepted, "failed for: EnrichmentFeedbackEntry.accepted");

        // AdaptiveThresholdState roundtrip
        let state = AdaptiveThresholdState {
            current_threshold: 0.72,
            last_analysis: None,
            threshold_history: vec![ThresholdChange {
                from: 0.7,
                to: 0.72,
                reason: "success rate increased".to_string(),
                timestamp: Utc::now(),
            }],
            updated_at: Utc::now(),
        };
        let json = serde_json::to_string(&state).unwrap();
        let loaded: AdaptiveThresholdState = serde_json::from_str(&json).unwrap();
        assert!((loaded.current_threshold - 0.72).abs() < f32::EPSILON);
        assert_eq!(loaded.threshold_history.len(), 1, "failed for: AdaptiveThresholdState.threshold_history");
    }

    #[test]
    fn test_outcome_record_has_no_tool_args_field() {
        // Privacy-by-omission: verify OutcomeRecord serialization
        // does NOT contain "tool_args" or "user_message" keys.
        let record = OutcomeRecord {
            id: "privacy-test".to_string(),
            session_key: "test:hash".to_string(),
            tool_name: "shell".to_string(),
            success: true,
            error_category: None,
            duration_ms: 100,
            confidence_score: None,
            confidence_dimensions: None,
            execution_mode: ExecutionMode::Chat,
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&record).unwrap();
        assert!(!json.contains("tool_args"));
        assert!(!json.contains("user_message"));
    }
}
