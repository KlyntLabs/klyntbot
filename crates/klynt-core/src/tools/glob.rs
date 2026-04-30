use crate::privacy::PrivacyGuard;
use async_trait::async_trait;
use common::{KlyntbotError, Result, ToolError};
use globset::{Glob, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tools_core::{RoutingContext, ToolExecute};
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct GlobArgs {
    /// Glob pattern relative to session cwd (e.g., `**/*.rs`, `src/**/*.ts`).
    #[param(required)]
    pub pattern: String,
    /// Optional root directory to search in (defaults to session cwd).
    pub path: Option<String>,
    /// Maximum number of paths to return (default 100).
    pub max_results: Option<u64>,
}

#[derive(ToolDerive)]
#[tool(
    name = "glob",
    description = "Find files matching a glob pattern under the session cwd. \
                   Returns one path per line, sorted by mtime descending. Privacy-excluded paths skipped.",
    params = "GlobArgs",
    permission = "read_only",
    category = "Search",
    cost = "Free",
    tags = "fs,search,coding",
    concurrency_safe = "true",
)]
pub struct GlobTool {
    cwd: PathBuf,
    privacy: Arc<PrivacyGuard>,
}

impl GlobTool {
    pub fn new(cwd: PathBuf, privacy: Arc<PrivacyGuard>) -> Self { Self { cwd, privacy } }
}

#[async_trait]
impl ToolExecute for GlobTool {
    type Params = GlobArgs;

    async fn execute(&self, args: GlobArgs, _ctx: &RoutingContext) -> Result<String> {
        let max = args.max_results.unwrap_or(100) as usize;
        let mut builder = GlobSetBuilder::new();
        builder.add(Glob::new(&args.pattern)
            .map_err(|e| KlyntbotError::Tool(ToolError::InvalidParams(format!("bad pattern: {e}"))))?);
        let set = builder.build()
            .map_err(|e| KlyntbotError::Tool(ToolError::InvalidParams(e.to_string())))?;

        let root = match args.path.as_deref() {
            Some(p) => {
                let resolved = self.cwd.join(p);
                let canonical = std::fs::canonicalize(&resolved).unwrap_or(resolved);
                if self.privacy.is_excluded(&canonical) {
                    return Err(KlyntbotError::Tool(ToolError::PermissionDenied(
                        format!("glob path '{}' is excluded by privacy policy", canonical.display()))));
                }
                canonical
            }
            None => self.cwd.clone(),
        };

        let privacy = self.privacy.clone();
        let matches = tokio::task::spawn_blocking(move || -> Vec<(std::time::SystemTime, PathBuf)> {
            let mut out: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
            for entry in WalkDir::new(&root).follow_links(false) {
                let Ok(entry) = entry else { continue };
                if !entry.file_type().is_file() { continue }
                let rel = entry.path().strip_prefix(&root).unwrap_or(entry.path());
                if !set.is_match(rel) { continue }
                if privacy.is_excluded(entry.path()) { continue }
                let mtime = entry.metadata().ok().and_then(|m| m.modified().ok()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                out.push((mtime, entry.path().to_path_buf()));
            }
            out.sort_by(|a, b| b.0.cmp(&a.0));
            out.truncate(max);
            out
        }).await.map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(e.to_string())))?;

        Ok(matches.into_iter()
            .map(|(_, p)| p.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("\n"))
    }
}
