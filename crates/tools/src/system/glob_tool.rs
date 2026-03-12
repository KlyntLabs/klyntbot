//! Glob tool for finding files by pattern matching.

use async_trait::async_trait;
use globset::Glob;
use std::path::PathBuf;
use walkdir::WalkDir;

use common::{Result, ToolError};
use tools_core::{RoutingContext, ToolParams};

use crate::filesystem::FsToolBase;

#[derive(Debug, ToolParams)]
pub struct GlobParams {
    /// Glob pattern like '**/*.rs', 'src/**/*.ts', '*.json'
    #[param(required)]
    pub pattern: String,

    /// Root directory to search from (default: workspace root)
    pub path: Option<String>,
}

#[derive(tools_core::Tool)]
#[tool(
    name = "glob",
    description = "Find files by glob pattern matching. Returns matching file paths sorted by modification time (most recent first).",
    params = "GlobParams",
    permission = "read_only",
    category = "Search",
    tags = "search,file,pattern",
    cost = "Free"
)]
pub struct GlobTool {
    base: FsToolBase,
}

impl GlobTool {
    pub fn new(allowed_dir: Option<PathBuf>) -> Self {
        Self {
            base: FsToolBase::new(allowed_dir),
        }
    }
}

#[async_trait]
impl tools_core::ToolExecute for GlobTool {
    type Params = GlobParams;

    async fn execute(&self, params: GlobParams, _ctx: &RoutingContext) -> Result<String> {
        let pattern_str = &params.pattern;

        let search_path = self.base.resolve_search_root(params.path.as_deref())?;

        let glob = Glob::new(pattern_str)
            .map_err(|e| ToolError::InvalidParams(format!("Invalid glob pattern: {}", e)))?
            .compile_matcher();

        let mut matches: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

        for entry in WalkDir::new(&search_path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let rel_path = entry
                .path()
                .strip_prefix(&search_path)
                .unwrap_or(entry.path());

            if glob.is_match(rel_path) {
                let mtime = entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                matches.push((rel_path.to_path_buf(), mtime));
            }
        }

        if matches.is_empty() {
            return Ok(format!(
                "No files found matching '{}' in {}",
                pattern_str,
                search_path.display()
            ));
        }

        // Sort by modification time (most recent first)
        matches.sort_by(|a, b| b.1.cmp(&a.1));

        let output: Vec<String> = matches
            .iter()
            .map(|(p, _)| p.to_string_lossy().to_string())
            .collect();

        Ok(output.join("\n"))
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
    async fn test_glob_basic() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.rs"), "").unwrap();
        fs::write(tmp.path().join("b.rs"), "").unwrap();
        fs::write(tmp.path().join("c.txt"), "").unwrap();

        let tool = GlobTool::new(Some(tmp.path().to_path_buf()));
        let result = tool
            .execute(
                serde_json::json!({
                    "pattern": "*.rs",
                    "path": tmp.path().to_str().unwrap()
                }),
                &test_ctx(),
            )
            .await
            .unwrap();

        assert!(result.contains("a.rs"));
        assert!(result.contains("b.rs"));
        assert!(!result.contains("c.txt"));
    }

    #[tokio::test]
    async fn test_glob_recursive() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(tmp.path().join("top.rs"), "").unwrap();
        fs::write(sub.join("nested.rs"), "").unwrap();

        let tool = GlobTool::new(Some(tmp.path().to_path_buf()));
        let result = tool
            .execute(
                serde_json::json!({
                    "pattern": "**/*.rs",
                    "path": tmp.path().to_str().unwrap()
                }),
                &test_ctx(),
            )
            .await
            .unwrap();

        assert!(result.contains("top.rs"));
        assert!(result.contains("nested.rs"));
    }

    #[tokio::test]
    async fn test_glob_no_matches() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), "").unwrap();

        let tool = GlobTool::new(Some(tmp.path().to_path_buf()));
        let result = tool
            .execute(
                serde_json::json!({
                    "pattern": "*.rs",
                    "path": tmp.path().to_str().unwrap()
                }),
                &test_ctx(),
            )
            .await
            .unwrap();

        assert!(result.contains("No files found"));
    }

    #[test]
    fn test_glob_schema() {
        let tool = GlobTool::new(None);
        let schema = tool.to_schema();
        assert_eq!(schema["function"]["name"], "glob");
    }
}
