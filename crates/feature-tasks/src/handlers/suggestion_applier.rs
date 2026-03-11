//! Suggestion applier handler trait.
//!
//! Defined here (Layer 4) for dependency inversion. The implementation
//! lives in the agent crate (Layer 5) and applies suggestion actions
//! to tasks (e.g., setting priority, triggering decomposition).

use async_trait::async_trait;
use common::Result;

use crate::types::SuggestionAction;

/// Handler for applying suggestion actions to tasks.
///
/// Takes a suggestion and its associated action, then modifies the
/// task accordingly. Returns a human-readable summary of what was changed.
#[async_trait]
pub trait SuggestionApplier: Send + Sync {
    /// Apply a suggestion's action to a task.
    ///
    /// - `suggestion_id`: The suggestion being applied (for audit trail).
    /// - `task_id`: The target task (if applicable; some suggestions are project-level).
    /// - `action`: The action to apply.
    ///
    /// Returns a human-readable summary of the changes made.
    async fn apply(
        &self,
        suggestion_id: &str,
        task_id: Option<&str>,
        action: &SuggestionAction,
    ) -> Result<String>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_suggestion_applier_is_object_safe() {
        fn _check(_: Arc<dyn SuggestionApplier>) {}
    }
}
