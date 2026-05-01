use crate::privacy::PrivacyGuard;
use crate::tools::shared::fs_resolve::resolve_under_cwd;
use crate::tools::shared::hook_emit::{fire_pre_tool_use, fire_post_tool_use};
use async_trait::async_trait;
use common::{KlyntbotError, Result, ToolError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tools_core::{RoutingContext, ToolExecute};
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct ReadArgs {
    /// File path; relative is resolved against session cwd, or absolute path inside cwd.
    #[param(required)]
    pub path: String,
    /// Optional 0-indexed line offset to start reading.
    pub offset: Option<u64>,
    /// Optional maximum number of lines to return.
    pub limit: Option<u64>,
}

#[derive(ToolDerive)]
#[tool(
    name = "read",
    description = "Read a UTF-8 text file from the session working directory. \
                   Returns the file contents (optionally sliced by offset/limit). \
                   Limited to files inside the session cwd; privacy guard applies.",
    params = "ReadArgs",
    permission = "read_only",
    category = "FileSystem",
    cost = "Free",
    tags = "fs,read,coding",
    concurrency_safe = "true",
)]
pub struct ReadTool {
    cwd: PathBuf,
    privacy: Arc<PrivacyGuard>,
}

impl ReadTool {
    pub fn new(cwd: PathBuf, privacy: Arc<PrivacyGuard>) -> Self { Self { cwd, privacy } }
}

#[async_trait]
impl ToolExecute for ReadTool {
    type Params = ReadArgs;

    async fn execute(&self, args: ReadArgs, ctx: &RoutingContext) -> Result<String> {
        let session_id = ctx.session_key.clone().map(|s| s.to_string()).unwrap_or_default();
        let args_json = serde_json::to_value(&args).unwrap_or_default();
        if let Err(reason) = fire_pre_tool_use(ctx.hook_engine.as_ref(), session_id.clone(), "read", args_json, None).await {
            return Err(KlyntbotError::Tool(ToolError::HookBlocked(reason)));
        }
        let start = std::time::Instant::now();
        let result: Result<String> = (async {
            let path = resolve_under_cwd(&args.path, &self.cwd, &self.privacy)
                .map_err(|e| KlyntbotError::Tool(ToolError::PermissionDenied(e.to_string())))?;

            let offset = args.offset.unwrap_or(0) as usize;
            let limit = args.limit.map(|l| l as usize);

            if offset == 0 && limit.is_none() {
                // Fast path: read whole file
                let bytes = tokio::fs::read(&path).await
                    .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("read {}: {e}", path.display()))))?;
                return Ok(String::from_utf8_lossy(&bytes).into_owned());
            }

            // Stream lines for large files when slicing
            let file = tokio::fs::File::open(&path).await
                .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("read {}: {e}", path.display()))))?;
            let reader = tokio::io::BufReader::new(file);
            let mut lines = tokio::io::AsyncBufReadExt::lines(reader);
            let mut out = String::new();
            let mut i = 0;
            let mut taken = 0;
            let max = limit.unwrap_or(usize::MAX);
            while let Some(line) = lines.next_line().await
                .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(format!("read {}: {e}", path.display()))))?
            {
                if i >= offset {
                    out.push_str(&line);
                    out.push('\n');
                    taken += 1;
                    if taken >= max {
                        break;
                    }
                }
                i += 1;
            }
            Ok(out)
        }).await;
        fire_post_tool_use(ctx.hook_engine.as_ref(), session_id, "read", result.is_ok(), start.elapsed().as_millis() as u64).await;
        result
    }
}
