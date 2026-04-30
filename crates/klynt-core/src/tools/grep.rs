use crate::privacy::PrivacyGuard;
use async_trait::async_trait;
use common::{KlyntbotError, Result, ToolError};
use regex::RegexBuilder;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tools_core::{RoutingContext, ToolExecute};
use tools_core_macros::{Tool as ToolDerive, ToolParams as ToolParamsDerive};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize, ToolParamsDerive)]
pub struct GrepArgs {
    /// Regex pattern (Rust regex syntax).
    #[param(required)] pub pattern: String,
    /// Optional file glob to restrict the search (default `**/*`).
    pub include: Option<String>,
    /// Case-insensitive match.
    pub case_insensitive: Option<bool>,
    /// Maximum result lines (default 200).
    pub max_results: Option<u64>,
    /// Lines of context around each match (0-5; clamped).
    pub context_lines: Option<u8>,
}

#[derive(ToolDerive)]
#[tool(
    name = "grep",
    description = "Search for a regex pattern across files under session cwd. \
                   Returns `path:line:text` lines. Privacy-excluded paths skipped.",
    params = "GrepArgs",
    permission = "read_only",
    category = "Search",
    cost = "Free",
    tags = "search,grep,coding",
    concurrency_safe = "true",
)]
pub struct GrepTool {
    cwd: PathBuf,
    privacy: Arc<PrivacyGuard>,
}

impl GrepTool {
    pub fn new(cwd: PathBuf, privacy: Arc<PrivacyGuard>) -> Self { Self { cwd, privacy } }
}

#[async_trait]
impl ToolExecute for GrepTool {
    type Params = GrepArgs;

    async fn execute(&self, args: GrepArgs, _ctx: &RoutingContext) -> Result<String> {
        let max = args.max_results.unwrap_or(200) as usize;
        let ctx = args.context_lines.unwrap_or(0).min(5) as usize;
        let re = RegexBuilder::new(&args.pattern)
            .case_insensitive(args.case_insensitive.unwrap_or(false))
            .build()
            .map_err(|e| KlyntbotError::Tool(ToolError::InvalidParams(format!("regex: {e}"))))?;
        let include = args.include.unwrap_or_else(|| "**/*".into());
        let glob = globset::Glob::new(&include)
            .map_err(|e| KlyntbotError::Tool(ToolError::InvalidParams(format!("include: {e}"))))?
            .compile_matcher();

        let cwd = self.cwd.clone();
        let privacy = self.privacy.clone();
        let lines = tokio::task::spawn_blocking(move || -> Vec<String> {
            let mut out: Vec<String> = Vec::new();
            'outer: for entry in WalkDir::new(&cwd).follow_links(false) {
                let Ok(entry) = entry else { continue };
                if !entry.file_type().is_file() { continue }
                let rel = entry.path().strip_prefix(&cwd).unwrap_or(entry.path());
                if !glob.is_match(rel) { continue }
                if privacy.is_excluded(entry.path()) { continue }
                let file = match std::fs::File::open(entry.path()) {
                    Ok(f) => f,
                    Err(_) => continue,
                };
                let reader = std::io::BufReader::new(file);
                let file_lines: Vec<String> = std::io::BufRead::lines(reader)
                    .filter_map(|l| l.ok())
                    .collect();
                let mut last_hi = 0usize;
                for (i, line) in file_lines.iter().enumerate() {
                    if re.is_match(line) {
                        let lo = i.saturating_sub(ctx);
                        let hi = (i + ctx + 1).min(file_lines.len());
                        if lo > last_hi && !out.is_empty() {
                            out.push("--".into());
                        }
                        for j in lo..hi {
                            let marker = if j == i { ":" } else { "-" };
                            out.push(format!("{}:{}{}:{}",
                                rel.display(), j + 1, marker, file_lines[j]));
                        }
                        last_hi = hi;
                        if out.len() >= max { break 'outer; }
                    }
                }
            }
            out
        }).await.map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(e.to_string())))?;
        Ok(lines.join("\n"))
    }
}
