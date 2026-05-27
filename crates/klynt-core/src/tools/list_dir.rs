//! List directory contents (klynt-core native impl).
//!
//! Replaces `tools::system::filesystem::ListDirTool` so the old system module
//! can be deleted in Task 9.

use crate::privacy::PrivacyGuard;
use crate::tools::shared::fs_resolve::resolve_under_cwd;
use crate::tools::shared::hook_emit::{fire_post_tool_use, fire_pre_tool_use};
use async_trait::async_trait;
use common::{KlyntbotError, Result, ToolError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tools_core::{HookCtx, ToolExecute};
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct ListDirArgs {
    /// Directory path; relative is resolved against session cwd.
    #[param(required)]
    pub path: String,
}

#[derive(ToolDerive)]
#[tool(
    name = "list_dir",
    description = "List the contents of a directory. Returns entries with 📁 prefix for directories and 📄 prefix for files.",
    params = "ListDirArgs",
    permission = "read_only",
    category = "FileSystem",
    cost = "Free",
    tags = "fs,list,directory",
    concurrency_safe = "true"
)]
pub struct ListDirTool {
    cwd: PathBuf,
    privacy: Arc<PrivacyGuard>,
}

impl ListDirTool {
    pub fn new(cwd: PathBuf, privacy: Arc<PrivacyGuard>) -> Self {
        Self { cwd, privacy }
    }
}

#[async_trait]
impl ToolExecute for ListDirTool {
    type Params = ListDirArgs;
    type Ctx<'a> = HookCtx;

    async fn execute<'c>(&self, args: ListDirArgs, ctx: HookCtx) -> Result<String> {
        let session_id = ctx
            .session_key
            .clone()
            .map(|s| s.to_string())
            .unwrap_or_default();
        if let Err(reason) = fire_pre_tool_use(
            ctx.hook_engine.as_ref(),
            session_id.clone(),
            "list_dir",
            &args,
            None,
        )
        .await
        {
            return Err(KlyntbotError::Tool(ToolError::HookBlocked(reason)));
        }
        let start = std::time::Instant::now();
        let result: Result<String> = (async {
            let dir_path = resolve_under_cwd(&args.path, &self.cwd, &self.privacy)
                .map_err(|e| KlyntbotError::Tool(ToolError::PermissionDenied(e.to_string())))?;

            let mut entries = match tokio::fs::read_dir(&dir_path).await {
                Ok(e) => e,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Err(KlyntbotError::Tool(ToolError::ExecutionFailed(format!(
                        "Directory not found: {}",
                        args.path
                    ))));
                }
                Err(e) if e.raw_os_error() == Some(20 /* ENOTDIR */) => {
                    return Err(KlyntbotError::Tool(ToolError::ExecutionFailed(format!(
                        "Not a directory: {}",
                        args.path
                    ))));
                }
                Err(e) => {
                    return Err(KlyntbotError::Tool(ToolError::ExecutionFailed(format!(
                        "Failed to read directory: {e}"
                    ))));
                }
            };

            let mut items = Vec::new();
            while let Some(entry) = entries.next_entry().await.map_err(|e| {
                KlyntbotError::Tool(ToolError::ExecutionFailed(format!(
                    "Failed to read directory entry: {e}"
                )))
            })? {
                let file_type = entry.file_type().await.map_err(|e| {
                    KlyntbotError::Tool(ToolError::ExecutionFailed(format!(
                        "Failed to read file type: {e}"
                    )))
                })?;
                let prefix = if file_type.is_dir() { "📁 " } else { "📄 " };
                let name = entry.file_name().to_string_lossy().to_string();
                items.push(format!("{}{}", prefix, name));
            }

            if items.is_empty() {
                return Ok(format!("Directory {} is empty", args.path));
            }

            items.sort();
            Ok(items.join("\n"))
        })
        .await;
        fire_post_tool_use(
            ctx.hook_engine.as_ref(),
            session_id,
            "list_dir",
            result.is_ok(),
            start.elapsed().as_millis() as u64,
        )
        .await;
        result
    }
}
