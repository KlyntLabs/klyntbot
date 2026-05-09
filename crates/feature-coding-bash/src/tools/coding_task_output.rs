use serde::Serialize;
use tools_core::{JobId, RoutingContext};
use tools_core_macros::{Tool, ToolParams};

#[derive(Debug, Clone, Serialize, ToolParams)]
pub struct CodingTaskOutputArgs {
    #[param(required)]
    pub task_id: String,
    pub since_offset: Option<u64>,
    pub block: Option<bool>,
    pub timeout_ms: Option<u64>,
}

#[derive(Tool)]
#[tool(
    name = "coding_task_output",
    description = "Read new output bytes from a background bash job since the given cursor offset.",
    params = "CodingTaskOutputArgs",
    allowed_channels = "coding_only",
    approval_class = "safe",
)]
pub struct CodingTaskOutputTool;

#[async_trait::async_trait]
impl tools_core::ToolExecute for CodingTaskOutputTool {
    type Params = CodingTaskOutputArgs;

    async fn execute(&self, args: Self::Params, ctx: &RoutingContext) -> common::Result<String> {
        let sup = ctx.job_supervisor.as_ref().ok_or_else(|| {
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(
                "background jobs disabled".into(),
            ))
        })?;
        let id = JobId::from_str(args.task_id)
            .map_err(|e| common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(format!("invalid task_id: {e}"))))?;
        let since = args.since_offset.unwrap_or(0);
        let block = args.block.unwrap_or(false);
        let timeout = args.timeout_ms.unwrap_or(30_000);
        let rd = sup
            .output_delta(&id, since, block, timeout)
            .await
            .map_err(|e| common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(format!("output_delta: {e}"))))?;
        let body = String::from_utf8_lossy(&rd.bytes);
        let trailer = serde_json::json!({
            "task_id": id.as_str(),
            "new_offset": rd.new_offset,
            "total_bytes_emitted": rd.total_bytes_emitted,
            "bisect_generation": rd.bisect_generation,
            "bisect_occurred_since": rd.bisect_occurred_since,
            "bytes_returned": rd.bytes.len(),
        });
        Ok(format!(
            "{body}\n\n[metadata: {}]",
            serde_json::to_string(&trailer).unwrap_or_default()
        ))
    }
}
