//! Feedback handler and helper utilities for the TodoTool.

use chrono::Utc;
use std::sync::Arc;
use tracing::warn;

use crate::enrichment::{EnrichmentFeedbackEntry, EnrichmentFeedbackHandler, EnrichmentResult};
use crate::tool::TaskTool;

impl TaskTool {
    /// Push a single task to calendar (best-effort).
    pub(crate) async fn push_task_to_calendar(&self, task_id: &str) {
        if let Some(handler) = &self.calendar_handler {
            if let Err(e) = handler.push_task(task_id).await {
                warn!("Calendar push for task {} failed: {}", task_id, e);
            }
        }
    }

    /// Remove a calendar event by UID (best-effort).
    pub(crate) async fn remove_event_from_calendar(&self, uid: &str) {
        if let Some(handler) = &self.calendar_handler {
            if let Err(e) = handler.remove_event(uid).await {
                warn!("Calendar event removal for {} failed: {}", uid, e);
            }
        }
    }

    /// Record enrichment feedback for all suggestion fields.
    ///
    /// When `accepted_override` is `Some(true)`, all fields are marked accepted.
    /// When `None`, each field's confidence is compared against 0.7 threshold.
    pub(crate) async fn record_enrichment_feedback(
        &self,
        task_id: &str,
        result: &EnrichmentResult,
        accepted_override: Option<bool>,
    ) {
        let Some(fb) = &self.feedback_handler else {
            return;
        };
        let now = Utc::now();

        let fields: Vec<(&str, String, f64)> = [
            result
                .priority
                .as_ref()
                .map(|s| ("priority", s.value.to_string(), s.confidence)),
            result
                .estimated_minutes
                .as_ref()
                .map(|s| ("estimated_minutes", s.value.to_string(), s.confidence)),
            result
                .due_date
                .as_ref()
                .map(|s| ("due_date", s.value.to_string(), s.confidence)),
        ]
        .into_iter()
        .flatten()
        .collect();

        for (field, suggested_value, conf) in fields {
            let accepted = accepted_override.unwrap_or(conf >= 0.7);
            let _ = fb
                .record_feedback(EnrichmentFeedbackEntry {
                    task_id: task_id.to_string(),
                    field: field.to_string(),
                    suggested_value,
                    actual_value: None,
                    accepted,
                    confidence: conf,
                    timestamp: now,
                })
                .await;
        }
    }

    /// Add an enrichment feedback handler.
    pub fn with_feedback_handler(mut self, handler: Arc<dyn EnrichmentFeedbackHandler>) -> Self {
        self.feedback_handler = Some(handler);
        self
    }
}
