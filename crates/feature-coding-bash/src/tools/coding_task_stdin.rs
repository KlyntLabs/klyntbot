use base64::engine::Engine;
use serde::Serialize;
use tools_core::{JobId, RoutingContext};
use tools_core_macros::{Tool, ToolParams};

#[derive(Debug, Clone, Serialize, ToolParams)]
pub struct CodingTaskStdinArgs {
    #[param(required)]
    pub task_id: String,
    /// Bytes to send. UTF-8 if encoding="utf8" (default), or base64-decoded
    /// if encoding="base64".
    #[param(required)]
    pub data: String,
    /// "utf8" (default) or "base64". Use base64 for control characters
    /// (Ctrl-C "\x03", Ctrl-D "\x04", arrow keys "\x1b[A").
    pub encoding: Option<String>,
}

#[derive(Tool)]
#[tool(
    name = "coding_task_stdin",
    description = "Send bytes to the stdin of a background PTY job. Use encoding=\"base64\" for control characters.",
    params = "CodingTaskStdinArgs",
    allowed_channels = "coding_only",
    approval_class = "sensitive",
    approval_scope = "command"
)]
pub struct CodingTaskStdinTool;

#[async_trait::async_trait]
impl tools_core::ToolExecute for CodingTaskStdinTool {
    type Params = CodingTaskStdinArgs;

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
        let encoding = args.encoding.as_deref().unwrap_or("utf8");
        let bytes = match encoding {
            "utf8" => args.data.into_bytes(),
            "base64" => base64::engine::general_purpose::STANDARD
                .decode(&args.data)
                .map_err(|e| {
                    common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(format!(
                        "invalid base64 payload: {e}"
                    )))
                })?,
            other => {
                return Err(common::KlyntbotError::Tool(
                    common::ToolError::ExecutionFailed(format!(
                        "unknown encoding {other:?}; use \"utf8\" or \"base64\""
                    )),
                ));
            }
        };
        let n = sup.write_stdin(&id, &bytes).await.map_err(|e| match e {
            tools_core::JobError::NotPty => common::KlyntbotError::Tool(
                common::ToolError::ExecutionFailed(
                    "job has no PTY; spawn with tty=true to enable stdin".into(),
                ),
            ),
            tools_core::JobError::NotFound(s) => common::KlyntbotError::Tool(
                common::ToolError::ExecutionFailed(format!("job not found: {s}")),
            ),
            other => common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(format!(
                "write_stdin: {other}"
            ))),
        })?;
        Ok(format!("Sent {n} bytes to {}.", id.as_str()))
    }
}
