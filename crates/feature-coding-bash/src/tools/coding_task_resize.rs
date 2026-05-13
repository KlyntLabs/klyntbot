use serde::Serialize;
use tools_core::{JobId, RoutingContext};
use tools_core_macros::{Tool, ToolParams};

#[derive(Debug, Clone, Serialize, ToolParams)]
pub struct CodingTaskResizeArgs {
    #[param(required)]
    pub task_id: String,
    #[param(required)]
    pub rows: u16,
    #[param(required)]
    pub cols: u16,
}

#[derive(Tool)]
#[tool(
    name = "coding_task_resize",
    description = "Resize the PTY of a background job. Sends SIGWINCH to the child.",
    params = "CodingTaskResizeArgs",
    allowed_channels = "coding_only",
    approval_class = "safe"
)]
pub struct CodingTaskResizeTool;

#[async_trait::async_trait]
impl tools_core::ToolExecute for CodingTaskResizeTool {
    type Params = CodingTaskResizeArgs;

    async fn execute(
        &self,
        args: Self::Params,
        ctx: &RoutingContext,
    ) -> common::Result<String> {
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
        sup.resize(&id, args.rows, args.cols).await.map_err(|e| match e {
            tools_core::JobError::NotPty => common::KlyntbotError::Tool(
                common::ToolError::ExecutionFailed("job has no PTY".into()),
            ),
            other => common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(format!(
                "resize: {other}"
            ))),
        })?;
        Ok(format!(
            "Resized {} to {} rows × {} cols.",
            id.as_str(),
            args.rows.clamp(tools_core::PTY_ROWS_MIN, tools_core::PTY_ROWS_MAX),
            args.cols.clamp(tools_core::PTY_COLS_MIN, tools_core::PTY_COLS_MAX)
        ))
    }
}
