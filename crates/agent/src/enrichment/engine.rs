//! EnrichmentEngine — orchestrator that implements EnrichmentHandler.

use async_trait::async_trait;
use common::Result;
use config::TodoEnrichmentConfig;
use feature_todo::{EnrichmentHandler, EnrichmentResult, Todo};

/// Central enrichment engine that coordinates priority, duration, and scheduling modules.
pub struct EnrichmentEngine {
    config: TodoEnrichmentConfig,
}

impl EnrichmentEngine {
    pub fn new(config: TodoEnrichmentConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl EnrichmentHandler for EnrichmentEngine {
    async fn enrich_task(&self, task: &Todo) -> Result<Option<EnrichmentResult>> {
        if !self.config.enabled {
            return Ok(None);
        }

        let mut result = EnrichmentResult::default();

        // Infer priority (only if not already set)
        if task.priority.is_none() {
            result.priority = super::priority::infer_priority(task);
        }

        // Predict duration (only if not already set)
        if task.estimated_minutes.is_none() {
            result.estimated_minutes = super::duration::predict_duration(task);
        }

        // Suggest due date (only if not already set)
        if task.due_date.is_none() {
            result.due_date = super::scheduling::suggest_due_date(task);
        }

        if result.is_empty() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use feature_todo::Todo;

    #[tokio::test]
    async fn test_enrichment_engine_disabled() {
        let config = TodoEnrichmentConfig {
            enabled: false,
            ..Default::default()
        };
        let engine = EnrichmentEngine::new(config);
        let task = Todo::default_instance();

        let result = engine.enrich_task(&task).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_enrichment_engine_skips_existing_fields() {
        let config = TodoEnrichmentConfig::default();
        let engine = EnrichmentEngine::new(config);

        let mut task = Todo::default_instance();
        task.priority = Some(1);
        task.estimated_minutes = Some(60);
        // due_date is still None — should still suggest

        let result = engine.enrich_task(&task).await.unwrap();
        // Priority and duration should be skipped (already set)
        if let Some(ref enrichment) = result {
            assert!(enrichment.priority.is_none());
            assert!(enrichment.estimated_minutes.is_none());
        }
    }

    #[tokio::test]
    async fn test_enrichment_engine_enriches_blank_task() {
        let config = TodoEnrichmentConfig::default();
        let engine = EnrichmentEngine::new(config);

        let mut task = Todo::default_instance();
        task.title = "Fix urgent production bug in auth".to_string();

        let result = engine.enrich_task(&task).await.unwrap();
        assert!(result.is_some());

        let enrichment = result.unwrap();
        // Priority should be inferred from "urgent" keyword
        assert!(enrichment.priority.is_some());
        // Duration should have a default estimate
        assert!(enrichment.estimated_minutes.is_some());
    }

    // ========================================================================
    // Additional integration tests (added by QA)
    // ========================================================================

    #[tokio::test]
    async fn test_enrichment_returns_none_when_all_fields_set() {
        let config = TodoEnrichmentConfig::default();
        let engine = EnrichmentEngine::new(config);

        let mut task = Todo::default_instance();
        task.title = "URGENT: Fix bug".to_string();
        task.priority = Some(1);
        task.estimated_minutes = Some(30);
        task.due_date = Some(chrono::Utc::now() + chrono::Duration::days(1));

        let result = engine.enrich_task(&task).await.unwrap();
        // All fields already set, should return None
        assert!(
            result.is_none(),
            "Should return None when all fields are already set"
        );
    }

    #[tokio::test]
    async fn test_enrichment_partial_fields() {
        let config = TodoEnrichmentConfig::default();
        let engine = EnrichmentEngine::new(config);

        let mut task = Todo::default_instance();
        task.title = "Fix typo in docs".to_string();
        task.priority = Some(4); // Already set
                                 // estimated_minutes and due_date are None

        let result = engine.enrich_task(&task).await.unwrap();
        assert!(result.is_some(), "Should enrich missing fields");

        let enrichment = result.unwrap();
        assert!(
            enrichment.priority.is_none(),
            "Priority already set, should skip"
        );
        assert!(
            enrichment.estimated_minutes.is_some(),
            "Should suggest duration"
        );
    }

    #[tokio::test]
    async fn test_enrichment_with_unicode_text() {
        let config = TodoEnrichmentConfig::default();
        let engine = EnrichmentEngine::new(config);

        let mut task = Todo::default_instance();
        task.title = "修复紧急bug (fix urgent bug)".to_string();

        let result = engine.enrich_task(&task).await.unwrap();
        assert!(result.is_some());

        let enrichment = result.unwrap();
        // Should still detect "urgent" and "bug" keywords despite Unicode
        assert!(
            enrichment.priority.is_some(),
            "Should handle Unicode and detect keywords"
        );
    }

    #[tokio::test]
    async fn test_enrichment_empty_vs_whitespace_title() {
        let config = TodoEnrichmentConfig::default();
        let engine = EnrichmentEngine::new(config);

        // Test 1: Completely empty title
        let mut task1 = Todo::default_instance();
        task1.title = "".to_string();

        let result1 = engine.enrich_task(&task1).await.unwrap();
        assert!(result1.is_some());
        let enrich1 = result1.unwrap();
        assert!(enrich1.priority.is_some());
        assert_eq!(
            enrich1.priority.as_ref().unwrap().value,
            3,
            "Empty title should default to medium priority"
        );

        // Test 2: Whitespace-only title
        let mut task2 = Todo::default_instance();
        task2.title = "   \t\n  ".to_string();

        let result2 = engine.enrich_task(&task2).await.unwrap();
        assert!(result2.is_some());
        let enrich2 = result2.unwrap();
        assert_eq!(
            enrich2.priority.as_ref().unwrap().value,
            3,
            "Whitespace title should default to medium priority"
        );
    }

    #[tokio::test]
    async fn test_enrichment_confidence_values() {
        let config = TodoEnrichmentConfig::default();
        let engine = EnrichmentEngine::new(config);

        let mut task = Todo::default_instance();
        task.title = "URGENT critical production issue".to_string();

        let result = engine.enrich_task(&task).await.unwrap();
        assert!(result.is_some());

        let enrichment = result.unwrap();
        if let Some(priority_sug) = enrichment.priority {
            assert!(
                priority_sug.confidence >= 0.80,
                "High-priority keywords should have high confidence: {}",
                priority_sug.confidence
            );
            assert!(
                !priority_sug.reasoning.is_empty(),
                "Should provide reasoning"
            );
        }
    }

    #[tokio::test]
    async fn test_enrichment_with_only_tags() {
        let config = TodoEnrichmentConfig::default();
        let engine = EnrichmentEngine::new(config);

        let mut task = Todo::default_instance();
        task.title = "Do something".to_string(); // Generic title
        task.tags = vec!["urgent".to_string(), "critical".to_string()];

        let result = engine.enrich_task(&task).await.unwrap();
        assert!(result.is_some());

        let enrichment = result.unwrap();
        assert!(
            enrichment.priority.is_some(),
            "Should detect keywords in tags"
        );
        assert_eq!(
            enrichment.priority.unwrap().value,
            1,
            "Should infer high priority from tags"
        );
    }
}
