//! Handler trait for AI-powered productivity features.
//! Implemented in the agent crate to avoid circular dependencies.

use async_trait::async_trait;

/// LLM-generated session summary.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionSummary {
    /// Short title, e.g. "Deep TypeScript refactoring"
    pub title: String,
    /// One-line description, e.g. "Focused on ActivityTrack component with minimal distractions"
    pub description: String,
}

#[async_trait]
pub trait ProductivityHandler: Send + Sync {
    /// Generate a natural language summary of the day's productivity data.
    async fn generate_daily_summary(&self, context: &str) -> common::Result<String>;

    /// Generate a narrative description of the day from structured context.
    async fn generate_narrative(&self, context: &str) -> common::Result<String>;

    /// Classify an activity into a category using AI when rules don't match.
    async fn classify_activity(
        &self,
        app: &str,
        title: &str,
        url: Option<&str>,
    ) -> common::Result<String>;

    /// Generate a short title + description for a completed productivity session.
    /// Context includes: duration, category, apps, quality, switches.
    /// Returns a JSON object: `{"title": "...", "description": "..."}`.
    async fn generate_session_summary(
        &self,
        context: &str,
    ) -> common::Result<SessionSummary> {
        // Default implementation: parse the context for a fallback
        let _ = context;
        Err(common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(
            "Session summary generation not implemented".into(),
        )))
    }
}
