use crate::privacy::PrivacyGuard;
use crate::tools::shared::hook_emit::{fire_post_tool_use, fire_pre_tool_use};
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
    #[param(required)]
    pub pattern: String,
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
    concurrency_safe = "true"
)]
pub struct GrepTool {
    cwd: PathBuf,
    privacy: Arc<PrivacyGuard>,
}

impl GrepTool {
    pub fn new(cwd: PathBuf, privacy: Arc<PrivacyGuard>) -> Self {
        Self { cwd, privacy }
    }
}

#[async_trait]
impl ToolExecute for GrepTool {
    type Params = GrepArgs;

    async fn execute(&self, args: GrepArgs, ctx: &RoutingContext) -> Result<String> {
        let session_id = ctx
            .session_key
            .clone()
            .map(|s| s.to_string())
            .unwrap_or_default();
        if let Err(reason) = fire_pre_tool_use(
            ctx.hook_engine.as_ref(),
            session_id.clone(),
            "grep",
            &args,
            None,
        )
        .await
        {
            return Err(KlyntbotError::Tool(ToolError::HookBlocked(reason)));
        }
        let start = std::time::Instant::now();
        let result: Result<String> = (async {
            let max = args.max_results.unwrap_or(200) as usize;
            let ctx_lines = args.context_lines.unwrap_or(0).min(5) as usize;
            let re = RegexBuilder::new(&args.pattern)
                .case_insensitive(args.case_insensitive.unwrap_or(false))
                .build()
                .map_err(|e| {
                    KlyntbotError::Tool(ToolError::InvalidParams(format!("regex: {e}")))
                })?;
            let include = args.include.unwrap_or_else(|| "**/*".into());
            let glob = globset::Glob::new(&include)
                .map_err(|e| {
                    KlyntbotError::Tool(ToolError::InvalidParams(format!("include: {e}")))
                })?
                .compile_matcher();

            let cwd = self.cwd.clone();
            let privacy = self.privacy.clone();
            let lines = tokio::task::spawn_blocking(move || -> Vec<String> {
                let mut out: Vec<String> = Vec::new();
                'outer: for entry in WalkDir::new(&cwd).follow_links(false) {
                    let Ok(entry) = entry else { continue };
                    if !entry.file_type().is_file() {
                        continue;
                    }
                    let rel = entry.path().strip_prefix(&cwd).unwrap_or(entry.path());
                    if !glob.is_match(rel) {
                        continue;
                    }
                    if privacy.is_excluded(entry.path()) {
                        continue;
                    }
                    let file = match std::fs::File::open(entry.path()) {
                        Ok(f) => f,
                        Err(_) => continue,
                    };
                    let reader = std::io::BufReader::new(file);
                    let mut last_hi = 0usize;
                    // Stream file line-by-line with a small buffer for context.
                    let mut buf: Vec<(usize, String)> = Vec::with_capacity(ctx_lines + 1);
                    let mut pending: Option<usize> = None; // index in buf of the match line
                    for (i, line) in std::io::BufRead::lines(reader)
                        .map_while(|l| l.ok())
                        .enumerate()
                    {
                        buf.push((i, line));

                        if let Some(mid) = pending {
                            // Collecting suffix lines after a match
                            if buf.len() > mid + ctx_lines {
                                let start = mid.saturating_sub(ctx_lines);
                                let end = (mid + ctx_lines + 1).min(buf.len());
                                let lo = buf[start].0;
                                let hi = buf[end - 1].0 + 1;
                                if lo > last_hi && !out.is_empty() {
                                    out.push("--".into());
                                }
                                for (j, line_text) in &buf[start..end] {
                                    let marker = if *j == buf[mid].0 { ":" } else { "-" };
                                    out.push(format!(
                                        "{}{}{}{}{}",
                                        rel.display(),
                                        marker,
                                        j + 1,
                                        marker,
                                        line_text
                                    ));
                                }
                                last_hi = hi;
                                buf.drain(..end);
                                pending = None;
                                if out.len() >= max {
                                    break 'outer;
                                }
                            }
                            continue;
                        }

                        let check_idx = buf.len() - 1;
                        if re.is_match(&buf[check_idx].1) {
                            if ctx_lines == 0 {
                                let (j, line_text) = &buf[check_idx];
                                if *j > last_hi && !out.is_empty() {
                                    out.push("--".into());
                                }
                                out.push(format!(
                                    "{}{}{}{}{}",
                                    rel.display(),
                                    ":",
                                    j + 1,
                                    ":",
                                    line_text
                                ));
                                last_hi = j + 1;
                                buf.clear();
                                if out.len() >= max {
                                    break 'outer;
                                }
                            } else {
                                pending = Some(check_idx);
                            }
                        } else if buf.len() > ctx_lines {
                            buf.remove(0);
                        }
                    }
                    // Flush any pending match at EOF
                    if let Some(mid) = pending {
                        let start = mid.saturating_sub(ctx_lines);
                        let end = buf.len();
                        let lo = buf[start].0;
                        if lo > last_hi && !out.is_empty() {
                            out.push("--".into());
                        }
                        for (j, line_text) in &buf[start..end] {
                            let marker = if *j == buf[mid].0 { ":" } else { "-" };
                            out.push(format!(
                                "{}{}{}{}{}",
                                rel.display(),
                                marker,
                                j + 1,
                                marker,
                                line_text
                            ));
                        }
                        if out.len() >= max {
                            break 'outer;
                        }
                    }
                }
                out
            })
            .await
            .map_err(|e| KlyntbotError::Tool(ToolError::ExecutionFailed(e.to_string())))?;
            Ok(lines.join("\n"))
        })
        .await;
        fire_post_tool_use(
            ctx.hook_engine.as_ref(),
            session_id,
            "grep",
            result.is_ok(),
            start.elapsed().as_millis() as u64,
        )
        .await;
        result
    }
}
