//! TaskEnrichmentAdapter — implements feature_tasks::EnrichmentHandler.
//!
//! Adapts the existing EnrichmentEngine (which operates on feature_todo::Todo)
//! to satisfy feature_tasks::EnrichmentHandler (which operates on feature_tasks::Task).
//! Bridges the two by constructing a minimal Action from the Task's common fields
//! and forwarding the enrichment call, then converting the result.

use async_trait::async_trait;
use common::Result;
use feature_tasks::{EnrichmentHandler, EnrichmentResult, EnrichmentSuggestion, Task};
use std::sync::Arc;

use crate::enrichment::EnrichmentEngine;
// Import the trait to bring enrich_task into scope
use feature_todo::EnrichmentHandler as LegacyEnrichmentHandler;

/// Wraps EnrichmentEngine to satisfy feature_tasks::EnrichmentHandler.
pub struct TaskEnrichmentAdapter {
    inner: Arc<EnrichmentEngine>,
}

impl TaskEnrichmentAdapter {
    pub fn new(inner: Arc<EnrichmentEngine>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl EnrichmentHandler for TaskEnrichmentAdapter {
    async fn enrich(&self, task: &Task) -> Result<Option<EnrichmentResult>> {
        // Build a feature_todo::Action from common Task fields so we can reuse
        // the existing keyword/LLM enrichment logic.
        let mut action = feature_todo::Action::default_instance();
        action.title = task.title.clone();
        action.description = task.description.clone();
        action.tags = task.tags.clone();
        action.priority = task.priority.map(|p| p as u8);
        action.due_date = task.due_date;
        action.estimated_minutes = task.estimated_minutes.map(|m| m as u32);

        // Delegate to the existing enrichment engine.
        let legacy_result: Option<feature_todo::EnrichmentResult> =
            LegacyEnrichmentHandler::enrich_task(self.inner.as_ref(), &action).await?;

        // Convert feature_todo::EnrichmentResult → feature_tasks::EnrichmentResult.
        Ok(legacy_result.map(|r| {
            let mut result = EnrichmentResult::default();

            if let Some(p) = r.priority {
                result.priority = Some(EnrichmentSuggestion {
                    value: p.value as i16,
                    confidence: p.confidence,
                    reasoning: p.reasoning,
                });
            }

            if let Some(d) = r.due_date {
                result.due_date = Some(EnrichmentSuggestion {
                    value: d.value.to_rfc3339(),
                    confidence: d.confidence,
                    reasoning: d.reasoning,
                });
            }

            // estimated_minutes is not part of feature_tasks::EnrichmentResult,
            // so we leave the remaining fields as None.

            result
        }))
    }
}
