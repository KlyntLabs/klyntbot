use crate::tools::shared::hook_emit::{fire_post_tool_use, fire_pre_tool_use};
use async_trait::async_trait;
use common::{KlyntbotError, Result, ToolError};
use serde::{Deserialize, Serialize};
use tools_core::{RoutingContext, ToolExecute};
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct ToolSearchArgs {
    /// Free-text query against tool descriptions.
    pub query: Option<String>,
    /// Maximum number of suggestions to return.
    pub max_results: Option<u64>,
}

#[derive(ToolDerive, Default)]
#[tool(
    name = "tool_search",
    description = "Search the curated tool registry for tools matching a query.",
    params = "ToolSearchArgs",
    permission = "read_only",
    category = "System",
    cost = "Free",
    tags = "tools,coding",
    concurrency_safe = "true"
)]
pub struct ToolSearchTool {
    pub effectiveness_scores: Option<std::collections::HashMap<String, f32>>,
}

impl ToolSearchTool {
    pub fn new() -> Self {
        Self { effectiveness_scores: None }
    }

    pub fn with_effectiveness(scores: std::collections::HashMap<String, f32>) -> Self {
        Self { effectiveness_scores: Some(scores) }
    }

    fn curated_meta() -> Vec<super::ToolMeta> {
        vec![
            super::ToolMeta { name: "bash".into(), aliases: vec![], description: "Run a shell command".into() },
            super::ToolMeta { name: "read".into(), aliases: vec![], description: "Read a file".into() },
            super::ToolMeta { name: "edit".into(), aliases: vec![], description: "Edit a file in place".into() },
            super::ToolMeta { name: "write".into(), aliases: vec![], description: "Write a file".into() },
            super::ToolMeta { name: "apply_patch".into(), aliases: vec![], description: "Apply a unified-diff patch".into() },
            super::ToolMeta { name: "glob".into(), aliases: vec![], description: "List files matching a glob pattern".into() },
            super::ToolMeta { name: "grep".into(), aliases: vec![], description: "Search file contents with regex".into() },
            super::ToolMeta { name: "list_dir".into(), aliases: vec![], description: "List directory contents".into() },
            super::ToolMeta { name: "ask_user".into(), aliases: vec![], description: "Ask the user a question".into() },
            super::ToolMeta { name: "web_fetch".into(), aliases: vec![], description: "Fetch a URL".into() },
            super::ToolMeta { name: "tool_search".into(), aliases: vec![], description: "Search available tools".into() },
            super::ToolMeta { name: "enter_plan_mode".into(), aliases: vec![], description: "Enter plan mode".into() },
            super::ToolMeta { name: "exit_plan_mode".into(), aliases: vec![], description: "Exit plan mode".into() },
        ]
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
        let result: Result<String> = (async {
            let query = args.query.as_deref().unwrap_or("");
            let top_n = args.max_results.unwrap_or(10) as usize;
            let meta = Self::curated_meta();
            let index = super::ToolIndex::build(&meta);
            let hits = match &self.effectiveness_scores {
                Some(scores) => index.search_with_effectiveness(query, top_n, scores),
                None => index.search(query, top_n),
            };
            Ok(serde_json::to_string(&hits).map_err(|e| ToolError::ExecutionFailed(e.to_string()))?)
        }).await;
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
