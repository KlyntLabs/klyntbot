use crate::tools::shared::hook_emit::{fire_post_tool_use, fire_pre_tool_use};
use async_trait::async_trait;
use common::{KlyntbotError, Result, ToolError};
use serde::{Deserialize, Serialize};
use tools_core::{RoutingContext, ToolExecute};
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct ToolSearchArgs {
    /// Free-text query against deferred-tool descriptions. Plan 3 stub ignores this.
    pub query: Option<String>,
    /// Maximum number of suggestions to return.
    pub max_results: Option<u64>,
}

#[derive(ToolDerive, Default)]
#[tool(
    name = "tool_search",
    description = "Phase 1 stub. Searches a 'deferred tool' set for tools that match \
                   a query. Returns an empty array; full implementation arrives in Phase 2 \
                   with Mirror per-skill effectiveness reranking.",
    params = "ToolSearchArgs",
    permission = "read_only",
    category = "System",
    cost = "Free",
    tags = "tools,coding,stub",
    concurrency_safe = "true"
)]
pub struct ToolSearchTool;

impl ToolSearchTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ToolExecute for ToolSearchTool {
    type Params = ToolSearchArgs;
    async fn execute(&self, args: ToolSearchArgs, ctx: &RoutingContext) -> Result<String> {
        let session_id = ctx
            .session_key
            .clone()
            .map(|s| s.to_string())
            .unwrap_or_default();
        if let Err(reason) = fire_pre_tool_use(
            ctx.hook_engine.as_ref(),
            session_id.clone(),
            "tool_search",
            &args,
            None,
        )
        .await
        {
            return Err(KlyntbotError::Tool(ToolError::HookBlocked(reason)));
        }
        let start = std::time::Instant::now();
        let result: Result<String> = Ok("[]".into());
        fire_post_tool_use(
            ctx.hook_engine.as_ref(),
            session_id,
            "tool_search",
            result.is_ok(),
            start.elapsed().as_millis() as u64,
        )
        .await;
        result
    }
}
