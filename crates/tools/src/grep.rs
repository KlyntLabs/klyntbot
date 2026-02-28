//! Grep tool for searching file contents by regex pattern.

use async_trait::async_trait;
use globset::{Glob, GlobMatcher};
use regex::Regex;
use serde_json::Value;
use std::path::PathBuf;
use walkdir::WalkDir;

use super::{PermissionLevel, RoutingContext, Tool};
use crate::params::ParamExtractor;
use common::{Result, ToolError};

use crate::filesystem::FsToolBase;

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
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search file contents using regex patterns within a directory scope. Returns matching lines with file path and line number."
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for in file contents"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (default: workspace root)"
                },
                "glob": {
                    "type": "string",
                    "description": "File filter pattern, e.g. '*.rs', '*.py'. Only files matching this pattern are searched."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of matches to return (default: 20)",
                    "minimum": 1,
                    "maximum": 100
                },
                "context_lines": {
                    "type": "integer",
                    "description": "Number of lines of context to show before and after each match (default: 0)",
                    "minimum": 0,
                    "maximum": 5
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let p = ParamExtractor::new(&args);
        let pattern_str = p.required_str("pattern")?;
        let max_results = p.i64_or("max_results", 20)? as usize;
        let context_lines = p.i64_or("context_lines", 0)? as usize;

        let re = Regex::new(pattern_str)
            .map_err(|e| ToolError::InvalidParams(format!("Invalid regex: {}", e)))?;

        let search_path = if let Some(path) = p.optional_str("path")? {
            self.base.resolve_path(path)?
        } else if let Some(ref dir) = self.base.allowed_dir() {
            dir.clone()
        } else {
            std::env::current_dir()
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
        };

        let glob_matcher: Option<GlobMatcher> = if let Some(glob_str) = p.optional_str("glob")? {
            Some(
                Glob::new(glob_str)
                    .map_err(|e| ToolError::InvalidParams(format!("Invalid glob: {}", e)))?
                    .compile_matcher(),
            )
        } else {
            None
        };

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

            if let Some(ref matcher) = glob_matcher {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if !matcher.is_match(name) {
                        continue;
                    }
                }
            }

            // Skip unreadable/binary files
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let lines: Vec<&str> = content.lines().collect();
            let rel_path = path
                .strip_prefix(&search_path)
                .unwrap_or(path)
                .to_string_lossy();

            for (i, line) in lines.iter().enumerate() {
                if re.is_match(line) {
                    if context_lines > 0 {
                        let start = i.saturating_sub(context_lines);
                        let end = (i + context_lines + 1).min(lines.len());
                        for (j, line_content) in lines[start..end].iter().enumerate() {
                            let line_num = start + j;
                            let marker = if line_num == i { ">" } else { " " };
                            results.push(format!(
                                "{}{}:{}:{}",
                                marker,
                                rel_path,
                                line_num + 1,
                                line_content
                            ));
                        }
                        results.push("--".to_string());
                    } else {
                        results.push(format!("{}:{}:{}", rel_path, i + 1, line));
                    }

                    match_count += 1;
                    if match_count >= max_results {
                        break;
                    }
                }
            }

            if match_count >= max_results {
                break;
            }
        }

        if results.is_empty() {
            Ok(format!(
                "No matches found for pattern '{}' in {}",
                pattern_str,
                search_path.display()
            ))
        } else {
            Ok(results.join("\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
