//! Decomposition handler trait for breaking tasks into subtasks.
//!
//! Defined here (Layer 4) for dependency inversion. The implementation
//! lives in the agent crate (Layer 5) and uses LLM calls to analyze
//! task structure and generate decomposition plans.

use async_trait::async_trait;
use common::Result;

use crate::types::{DecompositionContext, DecompositionResult, Task};

/// Handler for decomposing a task into subtasks.
///
/// Given a task and decomposition context, the handler analyzes the task
/// and produces a tree of planned subtasks with estimated effort,
/// energy levels, and dependencies.
#[async_trait]
pub trait DecompositionHandler: Send + Sync {
    /// Decompose a task into a tree of subtasks.
    ///
    /// The handler should:
    /// - Analyze the task title, description, and acceptance criteria
    /// - Consider existing subtasks to avoid duplicates
    /// - Respect max_depth and max_subtasks_per_level constraints
    /// - Assign energy levels and estimates to each subtask
    /// - Identify dependencies between subtasks
    ///
    /// Returns a `DecompositionResult` with the subtask tree, confidence
    /// score, reasoning, and any validation warnings.
    async fn decompose(
        &self,
        task: &Task,
        context: &DecompositionContext,
    ) -> Result<DecompositionResult>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_decomposition_handler_is_object_safe() {
        fn _check(_: Arc<dyn DecompositionHandler>) {}
    }
}
