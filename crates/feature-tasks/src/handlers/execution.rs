//! Task execution handler trait for agentic task execution.
//!
//! Defined here (Layer 4) for dependency inversion. The implementation
//! lives in the agent crate (Layer 5) and orchestrates LLM-driven
//! execution of agentic tasks.

use async_trait::async_trait;
use common::Result;

use crate::types::{ExecuteResult, ExecutionConfig, Task, TaskExecution};

/// Handler for executing agentic tasks.
///
/// Manages the lifecycle of task executions: starting, monitoring,
/// cancelling, and retrying. Handles budget limits, timeouts,
/// and approval workflows.
#[async_trait]
pub trait TaskExecutionHandler: Send + Sync {
    /// Start executing a task with the given configuration.
    ///
    /// If the task's agent_config requires approval, returns
    /// `ExecuteResult::AwaitingApproval` with a suggestion ID.
    /// Otherwise, returns `ExecuteResult::Started` with an execution ID.
    async fn execute(&self, task: &Task, config: &ExecutionConfig) -> Result<ExecuteResult>;

    /// Get the current state of an execution.
    async fn get_execution(&self, execution_id: &str) -> Result<TaskExecution>;

    /// Cancel a running or queued execution.
    async fn cancel_execution(&self, execution_id: &str) -> Result<()>;

    /// Retry a failed execution, respecting the retry policy.
    async fn retry_execution(&self, execution_id: &str) -> Result<ExecuteResult>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_task_execution_handler_is_object_safe() {
        fn _check(_: Arc<dyn TaskExecutionHandler>) {}
    }
}
