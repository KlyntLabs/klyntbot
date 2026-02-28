//! Grep tool for searching file contents by regex pattern.

use async_trait::async_trait;
use globset::{Glob, GlobMatcher};
use regex::Regex;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use walkdir::WalkDir;

use common::{Result, ToolError};
use tools_core::{RoutingContext, ToolParams};

use crate::filesystem::FsToolBase;

#[derive(Debug, ToolParams)]
pub struct GrepParams {
    /// Regex pattern to search for in file contents
    #[param(required)]
    pub pattern: String,

    /// Directory to search in (default: workspace root)
    pub path: Option<String>,

    /// File filter pattern, e.g. '*.rs', '*.py'. Only files matching this pattern are searched.
    pub glob: Option<String>,

    /// Maximum number of matches to return (default: 20)
    #[param(min = 1, max = 100)]
    pub max_results: Option<i64>,

    /// Number of lines of context to show before and after each match (default: 0)
    #[param(min = 0, max = 5)]
    pub context_lines: Option<i64>,
}

#[derive(tools_core::Tool)]
#[tool(
    name = "grep",
    description = "Search file contents using regex patterns within a directory scope. Returns matching lines with file path and line number.",
    params = "GrepParams",
    permission = "read_only"
)]
pub struct GrepTool {
    base: FsToolBase,
}

impl GrepTool {
    pub fn new(allowed_dir: Option<PathBuf>) -> Self {
        Self {
            base: FsToolBase::new(allowed_dir),
        }
    }
}

#[async_trait]
impl tools_core::ToolExecute for GrepTool {
    type Params = GrepParams;

    async fn execute(&self, params: GrepParams, _ctx: &RoutingContext) -> Result<String> {
        let pattern_str = &params.pattern;
        let max_results = params.max_results.unwrap_or(20) as usize;
        let context_lines = params.context_lines.unwrap_or(0) as usize;

        let re = Regex::new(pattern_str)
            .map_err(|e| ToolError::InvalidParams(format!("Invalid regex: {}", e)))?;

        let search_path = self.base.resolve_search_root(params.path.as_deref())?;

        let glob_matcher: Option<GlobMatcher> = if let Some(glob_str) = params.glob.as_deref() {
            Some(
                Glob::new(glob_str)
                    .map_err(|e| ToolError::InvalidParams(format!("Invalid glob: {}", e)))?
                    .compile_matcher(),
            )
        } else {
            None
        };

        let pattern_display = pattern_str.to_string();
        let search_display = search_path.display().to_string();

        // Run all filesystem I/O on a blocking thread to avoid stalling the Tokio runtime.
        let output = tokio::task::spawn_blocking(move || {
            let mut results = Vec::new();
            let mut match_count = 0;

            for entry in WalkDir::new(&search_path)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if !entry.file_type().is_file() {
                    continue;
                }

                let path = entry.path();
                let rel_path = path.strip_prefix(&search_path).unwrap_or(path);

                if let Some(ref matcher) = glob_matcher {
                    if !matcher.is_match(rel_path) {
                        continue;
                    }
                }

                let rel_path_str = rel_path.to_string_lossy().to_string();

                // Use BufReader for line-by-line reading instead of loading entire file.
                let file = match std::fs::File::open(path) {
                    Ok(f) => f,
                    Err(_) => continue,
                };
                let reader = BufReader::new(file);

                if context_lines > 0 {
                    // Context mode: need random access to nearby lines, so collect.
                    let lines: Vec<String> = reader.lines().map_while(|l| l.ok()).collect();
                    for (i, line) in lines.iter().enumerate() {
                        if re.is_match(line) {
                            let start = i.saturating_sub(context_lines);
                            let end = (i + context_lines + 1).min(lines.len());
                            for (j, line_content) in lines[start..end].iter().enumerate() {
                                let line_num = start + j;
                                let marker = if line_num == i { ">" } else { " " };
                                results.push(format!(
                                    "{}{}:{}:{}",
                                    marker,
                                    rel_path_str,
                                    line_num + 1,
                                    line_content
                                ));
                            }
                            results.push("--".to_string());

                            match_count += 1;
                            if match_count >= max_results {
                                break;
                            }
                        }
                    }
                } else {
                    // No context: stream lines without collecting.
                    for (i, line) in reader.lines().enumerate() {
                        let line = match line {
                            Ok(l) => l,
                            Err(_) => break, // Binary/unreadable file
                        };
                        if re.is_match(&line) {
                            results.push(format!("{}:{}:{}", rel_path_str, i + 1, line));
                            match_count += 1;
                            if match_count >= max_results {
                                break;
                            }
                        }
                    }
                }

                if match_count >= max_results {
                    break;
                }
            }

            results
        })
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Search task failed: {}", e)))?;

        if output.is_empty() {
            Ok(format!(
                "No matches found for pattern '{}' in {}",
                pattern_display, search_display
            ))
        } else {
            Ok(output.join("\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tool;
    use std::fs;
    use tempfile::TempDir;

    fn test_ctx() -> RoutingContext {
        RoutingContext::new("cli".into(), "test".into())
    }

    #[tokio::test]
    async fn test_grep_basic_match() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("hello.txt"),
            "hello world\ngoodbye world\nhello again",
        )
        .unwrap();

        let tool = GrepTool::new(Some(tmp.path().to_path_buf()));
        let result = tool
            .execute(
                serde_json::json!({
                    "pattern": "hello",
                    "path": tmp.path().to_str().unwrap()
                }),
                &test_ctx(),
            )
            .await
            .unwrap();

        assert!(result.contains("hello.txt:1:"));
        assert!(result.contains("hello.txt:3:"));
        assert!(!result.contains("goodbye"));
    }

    #[tokio::test]
    async fn test_grep_with_glob_filter() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("code.rs"), "fn main() {}").unwrap();
        fs::write(tmp.path().join("notes.txt"), "fn notes() {}").unwrap();

        let tool = GrepTool::new(Some(tmp.path().to_path_buf()));
        let result = tool
            .execute(
                serde_json::json!({
                    "pattern": "fn",
                    "path": tmp.path().to_str().unwrap(),
                    "glob": "*.rs"
                }),
                &test_ctx(),
            )
            .await
            .unwrap();

        assert!(result.contains("code.rs"));
        assert!(!result.contains("notes.txt"));
    }

    #[tokio::test]
    async fn test_grep_max_results() {
        let tmp = TempDir::new().unwrap();
        let content: String = (0..50).map(|i| format!("match line {}\n", i)).collect();
        fs::write(tmp.path().join("big.txt"), &content).unwrap();

        let tool = GrepTool::new(Some(tmp.path().to_path_buf()));
        let result = tool
            .execute(
                serde_json::json!({
                    "pattern": "match",
                    "path": tmp.path().to_str().unwrap(),
                    "max_results": 5
                }),
                &test_ctx(),
            )
            .await
            .unwrap();

        let match_count = result.lines().filter(|l| l.contains("big.txt")).count();
        assert_eq!(match_count, 5);
    }

    #[tokio::test]
    async fn test_grep_invalid_regex() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("test.txt"), "content").unwrap();

        let tool = GrepTool::new(Some(tmp.path().to_path_buf()));
        let result = tool
            .execute(
                serde_json::json!({
                    "pattern": "[invalid",
                    "path": tmp.path().to_str().unwrap()
                }),
                &test_ctx(),
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_grep_context_lines() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("ctx.txt"),
            "line1\nline2\nMATCH\nline4\nline5",
        )
        .unwrap();

        let tool = GrepTool::new(Some(tmp.path().to_path_buf()));
        let result = tool
            .execute(
                serde_json::json!({
                    "pattern": "MATCH",
                    "path": tmp.path().to_str().unwrap(),
                    "context_lines": 1
                }),
                &test_ctx(),
            )
            .await
            .unwrap();

        assert!(result.contains("line2"));
        assert!(result.contains("MATCH"));
        assert!(result.contains("line4"));
    }

    #[test]
    fn test_grep_schema() {
        let tool = GrepTool::new(None);
        let schema = tool.to_schema();
        assert_eq!(schema["function"]["name"], "grep");
    }
}
