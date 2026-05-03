use crate::AppCore;
use common::Result;

/// Result for coding_review_start.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewResult {
    pub review_id: String,
    pub thread_id: String,
    pub summary: String,
    pub issues: Vec<ReviewIssue>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewIssue {
    pub severity: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub description: String,
    pub suggestion: Option<String>,
}

impl AppCore {
    /// Start a code review pass on a coding thread.
    ///
    /// If `target` is provided, reviews specific files/diffs; otherwise reviews
    /// recent changes in the thread.
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_review_start(
        &self,
        thread_id: &str,
        target: Option<&str>,
        delivery: Option<&str>,
    ) -> Result<ReviewResult> {
        // Load the session to verify it exists
        let session = self
            .repos
            .sessions
            .get_session(thread_id)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        let _ = delivery; // "inline" vs "detached" — future: choose presentation mode

        // For now, return a stub review result. Full LLM-driven review will use
        // the agent loop with a review-specific system prompt.
        let review_id = uuid::Uuid::new_v4().to_string();

        let summary = if let Some(t) = target {
            format!("Review of {t} — no issues found (stub)")
        } else {
            "Review of recent changes — no issues found (stub)".to_string()
        };

        tracing::info!(
            thread_id = %thread_id,
            review_id = %review_id,
            target = ?target,
            "Review started (stub)"
        );

        Ok(ReviewResult {
            review_id,
            thread_id: session.key,
            summary,
            issues: vec![],
        })
    }
}
