//! Integration tests for Smart Enrichment Engine
//!
//! Simplified version following existing test patterns from todo.rs

use klyntbot::agent::enrichment::EnrichmentEngine;
use klyntbot::config::TodoEnrichmentConfig;
use klyntbot::tools::enrichment::EnrichmentHandler;
use klyntbot::tools::todo_store::TodoStore;
use klyntbot::tools::todo_types::Todo;
use tempfile::TempDir;

#[tokio::test]
async fn test_enrichment_engine_with_disabled_config_returns_none() {
    let config = TodoEnrichmentConfig {
        enabled: false,
        ..Default::default()
    };
    let engine = EnrichmentEngine::new(config);

    let mut task = Todo::default_instance();
    task.title = "URGENT: Fix critical bug".to_string();

    let result = engine.enrich_task(&task).await.unwrap();
    assert!(result.is_none(), "Disabled enrichment should return None");
}

#[tokio::test]
async fn test_enrichment_engine_skips_already_set_fields() {
    let config = TodoEnrichmentConfig::default();
    let engine = EnrichmentEngine::new(config);

    let mut task = Todo::default_instance();
    task.title = "URGENT task".to_string();
    task.priority = Some(3); // Already set
    task.estimated_minutes = Some(45); // Already set

    let result = engine.enrich_task(&task).await.unwrap();

    // Should still return Some because due_date is None
    if let Some(enrichment) = result {
        assert!(
            enrichment.priority.is_none(),
            "Should skip already-set priority"
        );
        assert!(
            enrichment.estimated_minutes.is_none(),
            "Should skip already-set duration"
        );
    }
}

#[tokio::test]
async fn test_enrichment_engine_full_enrichment() {
    let config = TodoEnrichmentConfig::default();
    let engine = EnrichmentEngine::new(config);

    let mut task = Todo::default_instance();
    task.title = "Fix urgent bug in production auth system".to_string();

    let result = engine.enrich_task(&task).await.unwrap();
    assert!(result.is_some(), "Should provide enrichment");

    let enrichment = result.unwrap();

    // Priority should be inferred from "urgent"
    assert!(
        enrichment.priority.is_some(),
        "Should infer priority from 'urgent' keyword"
    );
    if let Some(priority_sug) = enrichment.priority {
        assert_eq!(priority_sug.value, 1, "Urgent should map to priority 1");
        assert!(
            priority_sug.confidence >= 0.80,
            "Should have high confidence"
        );
    }

    // Duration should be estimated from "fix"
    assert!(
        enrichment.estimated_minutes.is_some(),
        "Should estimate duration"
    );
}

#[tokio::test]
async fn test_enrichment_with_conflicting_keywords() {
    let config = TodoEnrichmentConfig::default();
    let engine = EnrichmentEngine::new(config);

    let mut task = Todo::default_instance();
    // Has both "urgent" (P1) and "typo" (P4)
    task.title = "URGENT: Fix typo in critical docs".to_string();

    let result = engine.enrich_task(&task).await.unwrap();
    assert!(result.is_some());

    let enrichment = result.unwrap();
    if let Some(priority_sug) = enrichment.priority {
        // Should pick highest confidence keyword
        // "urgent" has 0.90 confidence, "typo" has 0.87
        assert_eq!(
            priority_sug.value, 1,
            "Should pick 'urgent' over 'typo' based on confidence"
        );
    }
}

#[tokio::test]
async fn test_enrichment_with_unicode_content() {
    let config = TodoEnrichmentConfig::default();
    let engine = EnrichmentEngine::new(config);

    let mut task = Todo::default_instance();
    task.title = "紧急修复 urgent bug 🔥".to_string();
    task.description = Some("Critical issue in 日本語 system".to_string());

    let result = engine.enrich_task(&task).await.unwrap();
    assert!(result.is_some(), "Should handle Unicode gracefully");

    let enrichment = result.unwrap();
    assert!(
        enrichment.priority.is_some(),
        "Should detect 'urgent' keyword despite Unicode"
    );
}

#[tokio::test]
async fn test_enrichment_returns_none_when_all_fields_present() {
    let config = TodoEnrichmentConfig::default();
    let engine = EnrichmentEngine::new(config);

    let mut task = Todo::default_instance();
    task.title = "URGENT: Complete task".to_string();
    task.priority = Some(1);
    task.estimated_minutes = Some(30);
    task.due_date = Some(chrono::Utc::now() + chrono::Duration::days(1));

    let result = engine.enrich_task(&task).await.unwrap();
    assert!(
        result.is_none(),
        "Should return None when all enrichable fields are already set"
    );
}

#[tokio::test]
async fn test_enrichment_with_only_tags() {
    let config = TodoEnrichmentConfig::default();
    let engine = EnrichmentEngine::new(config);

    let mut task = Todo::default_instance();
    task.title = "Generic task title".to_string(); // No keywords
    task.tags = vec!["urgent".to_string(), "production".to_string()];

    let result = engine.enrich_task(&task).await.unwrap();
    assert!(result.is_some());

    let enrichment = result.unwrap();
    assert!(
        enrichment.priority.is_some(),
        "Should detect keywords in tags"
    );
    if let Some(priority_sug) = enrichment.priority {
        assert_eq!(
            priority_sug.value, 1,
            "Should infer high priority from tags"
        );
    }
}

#[tokio::test]
async fn test_enrichment_confidence_scoring() {
    let config = TodoEnrichmentConfig::default();
    let engine = EnrichmentEngine::new(config);

    // Test 1: High confidence keyword
    let mut task1 = Todo::default_instance();
    task1.title = "CRITICAL production outage".to_string();

    let result1 = engine.enrich_task(&task1).await.unwrap();
    if let Some(enrichment) = result1 {
        if let Some(priority_sug) = enrichment.priority {
            assert!(
                priority_sug.confidence >= 0.85,
                "Critical keyword should have high confidence"
            );
            assert!(
                !priority_sug.reasoning.is_empty(),
                "Should provide reasoning"
            );
        }
    }

    // Test 2: Low confidence (no keywords)
    let mut task2 = Todo::default_instance();
    task2.title = "Do something".to_string();

    let result2 = engine.enrich_task(&task2).await.unwrap();
    if let Some(enrichment) = result2 {
        if let Some(priority_sug) = enrichment.priority {
            assert!(
                priority_sug.confidence <= 0.60,
                "Default priority should have low confidence"
            );
        }
    }
}

#[tokio::test]
async fn test_todo_store_integration_with_enrichment() {
    // Test that TodoStore can work with enriched tasks
    let temp_dir = TempDir::new().unwrap();
    let todo_path = temp_dir.path().join("todos.jsonl");
    let mut store = TodoStore::new(todo_path);

    let config = TodoEnrichmentConfig::default();
    let engine = EnrichmentEngine::new(config);

    // Create a task
    let mut task = Todo::default_instance();
    task.title = "Fix urgent bug".to_string();

    // Enrich it
    let enrichment_result = engine.enrich_task(&task).await.unwrap();
    assert!(enrichment_result.is_some());

    // Apply enrichment
    if let Some(enrichment) = enrichment_result {
        if let Some(priority_sug) = enrichment.priority {
            task.priority = Some(priority_sug.value);
        }
        if let Some(duration_sug) = enrichment.estimated_minutes {
            task.estimated_minutes = Some(duration_sug.value);
        }
    }

    // Store should handle enriched task
    let stored_task = store.add(task).await.unwrap();
    assert_eq!(stored_task.priority, Some(1));
    assert!(stored_task.estimated_minutes.is_some());

    // Verify persistence
    let retrieved = store.get(&stored_task.id).await.unwrap();
    assert!(retrieved.is_some());
    let retrieved_task = retrieved.unwrap();
    assert_eq!(retrieved_task.priority, Some(1));
}
