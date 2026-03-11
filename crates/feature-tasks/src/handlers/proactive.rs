//! Proactive suggestion handler trait.
//!
//! Defined here (Layer 4) for dependency inversion. The implementation
//! lives in the agent crate (Layer 5) and periodically scans tasks
//! to generate actionable suggestions.

use async_trait::async_trait;
use common::Result;

use crate::types::{SuggestionCandidate, SuggestionScope, SuggestionTrigger, Task};

/// Handler for generating proactive task suggestions.
///
/// Scans tasks based on triggers (overdue, stale, WIP limit exceeded, etc.)
/// and produces suggestion candidates that can be reviewed or auto-applied.
#[async_trait]
pub trait ProactiveHandler: Send + Sync {
    /// Generate suggestions for tasks matching the given scope.
    ///
    /// Performs a broad scan of tasks within the scope and generates
    /// suggestions based on all applicable triggers.
    async fn suggest(&self, scope: &SuggestionScope) -> Result<Vec<SuggestionCandidate>>;

    /// Evaluate a specific task against a specific trigger.
    ///
    /// More targeted than `suggest()` -- used when a specific event
    /// (e.g., execution failure, focus abandonment) triggers evaluation.
    async fn evaluate_task(
        &self,
        task: &Task,
        trigger: &SuggestionTrigger,
    ) -> Result<Vec<SuggestionCandidate>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_proactive_handler_is_object_safe() {
        fn _check(_: Arc<dyn ProactiveHandler>) {}
    }
}
