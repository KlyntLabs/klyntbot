//! Enrichment feedback trait for the learning system.
//!
//! Defined in tools (Layer 3), implemented in agent (Layer 5).
//! This follows the established dependency inversion pattern
//! (EnrichmentHandler, CalendarHandler, etc.).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use common::Result;

/// Feedback on an enrichment suggestion applied to a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentFeedbackEntry {
    pub task_id: String,
    /// Which field was enriched (e.g., "priority", "estimated_minutes", "due_date").
    pub field: String,
    /// JSON-serialized suggested value.
    pub suggested_value: String,
    /// JSON-serialized final value (`None` = accepted as-is).
    pub actual_value: Option<String>,
    pub accepted: bool,
    /// Original suggestion confidence.
    pub confidence: f64,
    pub timestamp: DateTime<Utc>,
}

/// Trait for tools to report enrichment feedback to the learning system.
#[async_trait]
pub trait EnrichmentFeedbackHandler: Send + Sync {
    /// Record enrichment feedback (called by TodoTool on task update).
    async fn record_feedback(&self, feedback: EnrichmentFeedbackEntry) -> Result<()>;
}
