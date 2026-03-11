//! Execute action: start agentic task execution.

use common::Result;
use tools_core::ParamExtractor;
use tracing::info;

use super::super::TaskTool;
use crate::types::ExecutionConfig;

impl TaskTool {
    pub(crate) async fn handle_execute(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let handler = self.execution_handler.as_ref().ok_or_else(|| {
            common::ToolError::ExecutionFailed("Execution handler not available".into())
        })?;

        let id = p.required_str("id")?;
        let task = self.require_task(id).await?;

        let config = ExecutionConfig {
            agent_profile: p.optional_str("agent_profile")?.map(String::from),
            max_cost_usd: p.optional_f64("max_cost")?,
            max_iterations: p.optional_u64("max_iterations")?.map(|n| n as u32),
            timeout_secs: p.optional_u64("timeout_secs")?,
            require_approval: p.optional_bool("require_approval")?.unwrap_or(false),
            ..Default::default()
        };

        info!(task_id = %id, "Executing task");

        let result = handler.execute(&task, &config).await?;

        match result {
            crate::types::ExecuteResult::Started { execution_id } => Ok(format!(
                "Execution started for '{}' (execution: {})",
                task.title, execution_id
            )),
            crate::types::ExecuteResult::AwaitingApproval { suggestion_id } => Ok(format!(
                "Execution of '{}' requires approval. Suggestion created: {}",
                task.title, suggestion_id,
            )),
        }
    }

    pub(crate) async fn handle_cancel_execution(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let handler = self.execution_handler.as_ref().ok_or_else(|| {
            common::ToolError::ExecutionFailed("Execution handler not available".into())
        })?;

        let execution_id = p.required_str("execution_id")?;
        handler.cancel_execution(execution_id).await?;
        Ok(format!("Execution {} cancelled", execution_id))
    }
}
