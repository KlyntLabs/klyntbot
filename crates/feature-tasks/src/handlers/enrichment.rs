//! Enrichment handler trait for dependency inversion.
//!
//! Defined here (Layer 4) so the TaskTool can request enrichment
//! without depending on the agent crate (Layer 5). Enhanced from
//! feature-todo's enrichment to include agentic task fields.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::types::{EnergyLevel, Task, TaskType};
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
#[serde(rename_all = "camelCase")]
pub struct EnrichmentResult {
    pub priority: Option<EnrichmentSuggestion<i16>>,
    pub due_date: Option<EnrichmentSuggestion<String>>,
    pub tags: Option<EnrichmentSuggestion<Vec<String>>>,
    pub project_id: Option<EnrichmentSuggestion<String>>,
    pub energy_level: Option<EnrichmentSuggestion<EnergyLevel>>,
    pub task_type: Option<EnrichmentSuggestion<TaskType>>,
    pub suggested_tags: Option<EnrichmentSuggestion<Vec<String>>>,
    pub acceptance_criteria: Option<EnrichmentSuggestion<String>>,
}

impl EnrichmentResult {
    /// True when all suggestion fields are None.
    pub fn is_empty(&self) -> bool {
        self.priority.is_none()
            && self.due_date.is_none()
            && self.tags.is_none()
            && self.project_id.is_none()
            && self.energy_level.is_none()
            && self.task_type.is_none()
            && self.suggested_tags.is_none()
            && self.acceptance_criteria.is_none()
    }
}

/// Trait for enrichment handlers.
/// Implemented by the EnrichmentEngine in the agent crate.
#[async_trait]
pub trait EnrichmentHandler: Send + Sync {
    /// Analyse a task and return enrichment suggestions.
    /// Returns `Ok(None)` when enrichment is disabled or not applicable.
    async fn enrich(&self, task: &Task) -> Result<Option<EnrichmentResult>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_enrichment_result_is_empty() {
        let empty = EnrichmentResult::default();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_enrichment_result_not_empty_with_priority() {
        let r = EnrichmentResult {
            priority: Some(EnrichmentSuggestion {
                value: 1i16,
                confidence: 0.9,
                reasoning: "urgent keyword".to_string(),
            }),
            ..Default::default()
        };
        assert!(!r.is_empty());
    }

    #[test]
    fn test_enrichment_result_not_empty_with_energy() {
        let r = EnrichmentResult {
            energy_level: Some(EnrichmentSuggestion {
                value: EnergyLevel::Deep,
                confidence: 0.8,
                reasoning: "complex task".to_string(),
            }),
            ..Default::default()
        };
        assert!(!r.is_empty());
    }

    #[test]
    fn test_serde_round_trip() {
        let r = EnrichmentResult {
            priority: Some(EnrichmentSuggestion {
                value: 2i16,
                confidence: 0.85,
                reasoning: "test".to_string(),
            }),
            energy_level: Some(EnrichmentSuggestion {
                value: EnergyLevel::High,
                confidence: 0.75,
                reasoning: "needs focus".to_string(),
            }),
            ..Default::default()
        };
        let json = serde_json::to_string(&r).unwrap();
        let parsed: EnrichmentResult = serde_json::from_str(&json).unwrap();
        assert!(parsed.priority.is_some());
        assert!(parsed.energy_level.is_some());
        assert!(parsed.due_date.is_none());
    }

    #[test]
    fn test_enrichment_handler_is_object_safe() {
        fn _check(_: Arc<dyn EnrichmentHandler>) {}
    }
}
