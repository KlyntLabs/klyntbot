// EnrichmentHandler - Dependency inversion trait for smart task enrichment
//
// Defined here in tools crate (Layer 3) so TodoTool can call it.
// Implemented by EnrichmentEngine in agent crate (Layer 5).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::todo_types::Todo;
use common::Result;

/// A single enrichment suggestion with confidence score and reasoning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentSuggestion<T> {
    pub value: T,
    /// Confidence score in range [0.0, 1.0].
    pub confidence: f64,
    /// Human-readable explanation of why this value was suggested.
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
    /// True when all suggestion fields are None (nothing to enrich).
    pub fn is_empty(&self) -> bool {
        self.priority.is_none() && self.estimated_minutes.is_none() && self.due_date.is_none()
    }
}

/// EnrichmentHandler trait for dependency inversion.
/// Implemented by EnrichmentEngine in agent crate (Layer 5).
/// Defined here in tools crate (Layer 3) to break circular dependency.
#[async_trait]
pub trait EnrichmentHandler: Send + Sync {
    /// Analyse a task and return enrichment suggestions.
    /// Returns `Ok(None)` when enrichment is disabled or not applicable.
    async fn enrich_task(&self, task: &Todo) -> Result<Option<EnrichmentResult>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_enrichment_result_is_empty() {
        let empty = EnrichmentResult::default();
        assert!(empty.is_empty());

        let with_priority = EnrichmentResult {
            priority: Some(EnrichmentSuggestion {
                value: 2,
                confidence: 0.9,
                reasoning: "Contains 'urgent' keyword".to_string(),
            }),
            ..Default::default()
        };
        assert!(!with_priority.is_empty());
    }

    #[test]
    fn test_enrichment_suggestion_serde_round_trip() {
        let suggestion = EnrichmentSuggestion {
            value: 42u32,
            confidence: 0.87,
            reasoning: "Based on similar tasks".to_string(),
        };

        let json = serde_json::to_string(&suggestion).unwrap();
        let parsed: EnrichmentSuggestion<u32> = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.value, 42);
        assert!((parsed.confidence - 0.87).abs() < f64::EPSILON);
        assert_eq!(parsed.reasoning, "Based on similar tasks");
    }

    #[test]
    fn test_enrichment_result_serde_round_trip() {
        let result = EnrichmentResult {
            priority: Some(EnrichmentSuggestion {
                value: 1,
                confidence: 0.95,
                reasoning: "High priority".to_string(),
            }),
            estimated_minutes: Some(EnrichmentSuggestion {
                value: 30,
                confidence: 0.80,
                reasoning: "Small task".to_string(),
            }),
            due_date: Some(EnrichmentSuggestion {
                value: Utc::now(),
                confidence: 0.70,
                reasoning: "Deadline mentioned".to_string(),
            }),
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: EnrichmentResult = serde_json::from_str(&json).unwrap();

        assert!(parsed.priority.is_some());
        assert!(parsed.estimated_minutes.is_some());
        assert!(parsed.due_date.is_some());
    }

    // Verify the trait is object-safe (can be used as Arc<dyn EnrichmentHandler>)
    struct MockEnrichmentHandler;

    #[async_trait]
    impl EnrichmentHandler for MockEnrichmentHandler {
        async fn enrich_task(&self, _task: &Todo) -> Result<Option<EnrichmentResult>> {
            Ok(Some(EnrichmentResult {
                priority: Some(EnrichmentSuggestion {
                    value: 2,
                    confidence: 0.9,
                    reasoning: "Mock enrichment".to_string(),
                }),
                ..Default::default()
            }))
        }
    }

    #[tokio::test]
    async fn test_enrichment_handler_trait() {
        let handler = MockEnrichmentHandler;
        let task = Todo::default_instance();

        let result = handler.enrich_task(&task).await.unwrap();
        assert!(result.is_some());

        let enrichment = result.unwrap();
        assert_eq!(enrichment.priority.unwrap().value, 2);
    }

    #[tokio::test]
    async fn test_enrichment_handler_as_arc_dyn() {
        use std::sync::Arc;

        let handler: Arc<dyn EnrichmentHandler> = Arc::new(MockEnrichmentHandler);
        let task = Todo::default_instance();

        let result = handler.enrich_task(&task).await.unwrap();
        assert!(result.is_some());
    }
}
