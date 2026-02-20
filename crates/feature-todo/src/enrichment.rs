//! Enrichment handler trait for dependency inversion.
//!
//! Defined here (Layer 3) so TodoTool can request enrichment
//! without depending on the agent crate (Layer 5).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::Todo;
use common::Result;

/// A single enrichment suggestion with a confidence score and reasoning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentSuggestion<T> {
    pub value: T,
    /// Confidence score in [0.0, 1.0].
    pub confidence: f64,
    /// Human-readable explanation.
    pub reasoning: String,
}

/// Aggregated enrichment results for a single task.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnrichmentResult {
    pub priority: Option<EnrichmentSuggestion<u8>>,
    pub estimated_minutes: Option<EnrichmentSuggestion<u32>>,
    pub due_date: Option<EnrichmentSuggestion<DateTime<Utc>>>,
}

impl EnrichmentResult {
    /// True when all suggestion fields are None.
    pub fn is_empty(&self) -> bool {
        self.priority.is_none() && self.estimated_minutes.is_none() && self.due_date.is_none()
    }
}

/// Trait for enrichment handlers.
/// Implemented by the EnrichmentEngine in the agent crate.
#[async_trait]
pub trait EnrichmentHandler: Send + Sync {
    /// Analyse a task and return enrichment suggestions.
    /// Returns `Ok(None)` when enrichment is disabled or not applicable.
    async fn enrich_task(&self, task: &Todo) -> Result<Option<EnrichmentResult>>;
}

/// Feedback entry for recording enrichment outcomes.
#[derive(Debug, Clone)]
pub struct EnrichmentFeedbackEntry {
    pub task_id: String,
    pub field: String,
    pub suggested_value: String,
    pub actual_value: Option<String>,
    pub accepted: bool,
    pub confidence: f64,
    pub timestamp: DateTime<Utc>,
}

/// Trait for recording enrichment feedback.
#[async_trait]
pub trait EnrichmentFeedbackHandler: Send + Sync {
    async fn record_feedback(&self, entry: EnrichmentFeedbackEntry) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enrichment_result_is_empty() {
        let empty = EnrichmentResult::default();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_enrichment_result_not_empty_with_priority() {
        let r = EnrichmentResult {
            priority: Some(EnrichmentSuggestion {
                value: 1u8,
                confidence: 0.9,
                reasoning: "urgent keyword".to_string(),
            }),
            ..Default::default()
        };
        assert!(!r.is_empty());
    }

    #[test]
    fn test_serde_round_trip() {
        let r = EnrichmentResult {
            priority: Some(EnrichmentSuggestion {
                value: 2u8,
                confidence: 0.85,
                reasoning: "test".to_string(),
            }),
            estimated_minutes: Some(EnrichmentSuggestion {
                value: 30u32,
                confidence: 0.75,
                reasoning: "small task".to_string(),
            }),
            due_date: None,
        };
        let json = serde_json::to_string(&r).unwrap();
        let parsed: EnrichmentResult = serde_json::from_str(&json).unwrap();
        assert!(parsed.priority.is_some());
        assert!(parsed.estimated_minutes.is_some());
        assert!(parsed.due_date.is_none());
    }
}
