use serde::Serialize;
use tools_core::RoutingContext;
use tools_core_macros::{Tool, ToolParams};

#[derive(Debug, Clone, Serialize, ToolParams)]
pub struct CodingTaskListArgs {
    pub active_only: Option<bool>,
}

#[derive(Tool)]
#[tool(
    name = "coding_task_list",
    description = "List background bash jobs in the current thread.",
    params = "CodingTaskListArgs",
    allowed_channels = "coding_only",
    approval_class = "safe"
)]
pub struct CodingTaskListTool;

#[async_trait::async_trait]
impl tools_core::ToolExecute for CodingTaskListTool {
    type Params = CodingTaskListArgs;

    async fn execute(&self, args: Self::Params, ctx: &RoutingContext) -> common::Result<String> {
        let sup = ctx.job_supervisor.as_ref().ok_or_else(|| {
            common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(
                "background jobs disabled".into(),
            ))
        })?;
        let active_only = args.active_only.unwrap_or(true);
        let chain: &[String] = &ctx.agent_chain;
        let jobs = sup.list(ctx.chat_id.as_str(), chain, active_only).await;
        if jobs.is_empty() {
            return Ok(if active_only {
                "No active background jobs in this thread.".into()
            } else {
                "No background jobs in this thread.".into()
            });
        }
        let mut lines = Vec::with_capacity(jobs.len() * 2);
        for j in jobs {
            lines.push(format!(
                "{}  {:?}  {}  {}  {}",
                j.id.as_str(),
                j.status,
                j.started_at,
                crate::render::human_bytes(j.total_bytes_emitted),
                j.command,
            ));
            lines.push(format!(
                "  ({} bytes, last_seen_offset={})",
                j.total_bytes_emitted, j.last_seen_offset,
            ));
        }
        Ok(lines.join("\n"))
    }
}
