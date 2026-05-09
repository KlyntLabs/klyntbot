use serde::Serialize;
use tools_core::{JobId, RoutingContext};
use tools_core_macros::{Tool, ToolParams};

#[derive(Debug, Clone, Serialize, ToolParams)]
pub struct CodingTaskStopArgs {
    #[param(required)]
    pub task_id: String,
    pub reason: Option<String>,
}

#[derive(Tool)]
#[tool(
    name = "coding_task_stop",
    description = "Terminate a background bash job (SIGTERM, then SIGKILL after 2s grace).",
    params = "CodingTaskStopArgs",
    allowed_channels = "coding_only",
    approval_class = "sensitive"
)]
pub struct CodingTaskStopTool;

#[async_trait::async_trait]
impl tools_core::ToolExecute for CodingTaskStopTool {
    type Params = CodingTaskStopArgs;

    async fn execute(&self, args: Self::Params, ctx: &RoutingContext) -> common::Result<String> {
        let sup = ctx.job_supervisor.as_ref().ok_or_else(|| {
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(
                "background jobs disabled".into(),
            ))
        })?;
        let id = JobId::from_str(args.task_id).map_err(|e| {
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(format!(
                "invalid task_id: {e}"
            )))
        })?;
        let reason = args.reason.unwrap_or_else(|| "Stopped by LLM".into());
        let view = sup.stop(&id, &reason).await.map_err(|e| {
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(format!("stop: {e}")))
        })?;
        Ok(format!(
            "Stopped {} (reason: {}). Final summary at coding_task_output(\"{}\").",
            view.id.as_str(),
            reason,
            view.id.as_str(),
        ))
    }
}
