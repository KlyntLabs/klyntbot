//! Filesystem tools: read, write, edit, list.

use async_trait::async_trait;
use serde_json::Value;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::debug;

use super::{Tool, RoutingContext};
use klyntbot_core::{Result, ToolError};

/// Resolve path and optionally enforce directory restriction
fn resolve_path(path: &str, allowed_dir: Option<&PathBuf>) -> Result<PathBuf> {
    let expanded = shellexpand::tilde(path);
    let resolved = Path::new(expanded.as_ref())
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(expanded.as_ref()));

    if let Some(allowed) = allowed_dir {
        let allowed_resolved = allowed.canonicalize().unwrap_or_else(|_| allowed.clone());
        let resolved_str = resolved.to_string_lossy();
        let allowed_str = allowed_resolved.to_string_lossy();

        if !resolved_str.starts_with(allowed_str.as_ref()) {
            return Err(ToolError::PermissionDenied(format!(
                "Path {} is outside allowed directory {}",
                path, allowed_str
            ))
            .into());
        }
    }

    Ok(resolved)
}

/// Common fields for filesystem tools that need directory restriction.
struct FsToolBase {
    allowed_dir: Option<PathBuf>,
}

impl FsToolBase {
    fn new(allowed_dir: Option<PathBuf>) -> Self {
        Self { allowed_dir }
    }

    fn resolve_path(&self, path: &str) -> Result<PathBuf> {
        resolve_path(path, self.allowed_dir.as_ref())
    }
}

/// Tool to read file contents
pub struct ReadFileTool {
    base: FsToolBase,
}

impl ReadFileTool {
    pub fn new(allowed_dir: Option<PathBuf>) -> Self {
        Self {
            base: FsToolBase::new(allowed_dir),
        }
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file at the given path."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The file path to read"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("missing 'path' parameter".to_string()))?;

        debug!("Reading file: {}", path);

        let file_path = self.base.resolve_path(path)?;

        if !file_path.exists() {
            return Err(ToolError::ExecutionFailed(format!("File not found: {}", path)).into());
        }

        if !file_path.is_file() {
            return Err(ToolError::ExecutionFailed(format!("Not a file: {}", path)).into());
        }

        fs::read_to_string(&file_path)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Error reading file: {}", e)).into())
    }
}

/// Tool to write content to a file
pub struct WriteFileTool {
    base: FsToolBase,
}

impl WriteFileTool {
    pub fn new(allowed_dir: Option<PathBuf>) -> Self {
        Self {
            base: FsToolBase::new(allowed_dir),
        }
    }
}

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file at the given path. Creates parent directories if needed."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The file path to write to"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write"
                }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("missing 'path' parameter".to_string()))?;

        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("missing 'content' parameter".to_string()))?;

        debug!("Writing file: {}", path);

        let file_path = self.base.resolve_path(path)?;

        // Create parent directories
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to create parent directories: {}", e))
            })?;
        }

        fs::write(&file_path, content)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Error writing file: {}", e)))?;

        Ok(format!(
            "Successfully wrote {} bytes to {}",
            content.len(),
            path
        ))
    }
}

/// Tool to edit a file by replacing text
pub struct EditFileTool {
    base: FsToolBase,
}

impl EditFileTool {
    pub fn new(allowed_dir: Option<PathBuf>) -> Self {
        Self {
            base: FsToolBase::new(allowed_dir),
        }
    }
}

#[async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing old_text with new_text. The old_text must exist exactly in the file."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The file path to edit"
                },
                "old_text": {
                    "type": "string",
                    "description": "The exact text to find and replace"
                },
                "new_text": {
                    "type": "string",
                    "description": "The text to replace with"
                }
            },
            "required": ["path", "old_text", "new_text"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("missing 'path' parameter".to_string()))?;

        let old_text = args
            .get("old_text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("missing 'old_text' parameter".to_string()))?;

        let new_text = args
            .get("new_text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("missing 'new_text' parameter".to_string()))?;

        debug!("Editing file: {}", path);

        let file_path = self.base.resolve_path(path)?;

        if !file_path.exists() {
            return Err(ToolError::ExecutionFailed(format!("File not found: {}", path)).into());
        }

        let content = fs::read_to_string(&file_path)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read file: {}", e)))?;

        if !content.contains(old_text) {
            return Err(ToolError::ExecutionFailed(
                "old_text not found in file. Make sure it matches exactly.".to_string(),
            )
            .into());
        }

        // Count occurrences
        let count = content.matches(old_text).count();
        if count > 1 {
            return Err(ToolError::ExecutionFailed(format!(
                "old_text appears {} times. Please provide more context to make it unique.",
                count
            ))
            .into());
        }

        let new_content = content.replacen(old_text, new_text, 1);

        fs::write(&file_path, new_content)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write file: {}", e)))?;

        Ok(format!("Successfully edited {}", path))
    }
}

/// Tool to list directory contents
pub struct ListDirTool {
    base: FsToolBase,
}

impl ListDirTool {
    pub fn new(allowed_dir: Option<PathBuf>) -> Self {
        Self {
            base: FsToolBase::new(allowed_dir),
        }
    }
}

#[async_trait]
impl Tool for ListDirTool {
    fn name(&self) -> &str {
        "list_dir"
    }

    fn description(&self) -> &str {
        "List the contents of a directory."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The directory path to list"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("missing 'path' parameter".to_string()))?;

        debug!("Listing directory: {}", path);

        let dir_path = self.base.resolve_path(path)?;

        if !dir_path.exists() {
            return Err(
                ToolError::ExecutionFailed(format!("Directory not found: {}", path)).into(),
            );
        }

        if !dir_path.is_dir() {
            return Err(ToolError::ExecutionFailed(format!("Not a directory: {}", path)).into());
        }

        let mut entries = fs::read_dir(&dir_path)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read directory: {}", e)))?;

        let mut items = Vec::new();

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to read directory entry: {}", e))
        })? {
            let metadata = entry.metadata().await.map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to read metadata: {}", e))
            })?;

            let prefix = if metadata.is_dir() { "📁 " } else { "📄 " };
            let name = entry.file_name().to_string_lossy().to_string();
            items.push(format!("{}{}", prefix, name));
        }

        if items.is_empty() {
            return Ok(format!("Directory {} is empty", path));
        }

        items.sort();
        Ok(items.join("\n"))
    }
}

/// Convenience function to register all filesystem tools at once.
/// Reduces duplication in agent_loop.rs and subagent.rs.
pub fn register_fs_tools(
    registry: &mut crate::registry::ToolRegistry,
    allowed_dir: Option<PathBuf>,
) {
    registry.register(ReadFileTool::new(allowed_dir.clone()));
    registry.register(WriteFileTool::new(allowed_dir.clone()));
    registry.register(EditFileTool::new(allowed_dir.clone()));
    registry.register(ListDirTool::new(allowed_dir));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RoutingContext;
    use tempfile::TempDir;

    fn test_ctx() -> RoutingContext {
        RoutingContext::new("cli".into(), "test".into())
    }

    #[tokio::test]
    async fn test_read_file_tool() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        tokio::fs::write(&file_path, "Hello, World!").await.unwrap();

        let tool = ReadFileTool::new(None);
        let args = serde_json::json!({
            "path": file_path.to_str().unwrap()
        });

        let result = tool.execute(args, &test_ctx()).await.unwrap();
        assert_eq!(result, "Hello, World!");
    }

    #[tokio::test]
    async fn test_read_file_not_found() {
        let tool = ReadFileTool::new(None);
        let args = serde_json::json!({
            "path": "/tmp/nonexistent_file_12345.txt"
        });

        let result = tool.execute(args, &test_ctx()).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("File not found"));
    }

    #[tokio::test]
    async fn test_read_file_with_directory_restriction() {
        let temp_dir = TempDir::new().unwrap();
        let allowed_dir = temp_dir.path().to_path_buf();

        // Create a file inside allowed directory
        let file_path = temp_dir.path().join("allowed.txt");
        tokio::fs::write(&file_path, "Allowed content").await.unwrap();

        let tool = ReadFileTool::new(Some(allowed_dir.clone()));

        // Should succeed for file inside allowed directory
        let args = serde_json::json!({
            "path": file_path.to_str().unwrap()
        });
        let result = tool.execute(args, &test_ctx()).await.unwrap();
        assert_eq!(result, "Allowed content");
    }

    #[tokio::test]
    async fn test_write_file_tool() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("output.txt");

        let tool = WriteFileTool::new(None);
        let args = serde_json::json!({
            "path": file_path.to_str().unwrap(),
            "content": "Test content"
        });

        let result = tool.execute(args, &test_ctx()).await.unwrap();
        assert!(result.contains("Successfully wrote"));

        // Verify file was created with correct content
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "Test content");
    }

    #[tokio::test]
    async fn test_write_file_creates_parent_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir
            .path()
            .join("subdir")
            .join("nested")
            .join("file.txt");

        let tool = WriteFileTool::new(None);
        let args = serde_json::json!({
            "path": file_path.to_str().unwrap(),
            "content": "Nested content"
        });

        let result = tool.execute(args, &test_ctx()).await.unwrap();
        assert!(result.contains("Successfully wrote"));

        // Verify parent directories were created
        assert!(file_path.parent().unwrap().exists());
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "Nested content");
    }

    #[tokio::test]
    async fn test_edit_file_tool() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("edit.txt");
        tokio::fs::write(&file_path, "Hello World\nGoodbye World")
            .await
            .unwrap();

        let tool = EditFileTool::new(None);
        let args = serde_json::json!({
            "path": file_path.to_str().unwrap(),
            "old_text": "Hello",
            "new_text": "Hi"
        });

        let result = tool.execute(args, &test_ctx()).await.unwrap();
        assert!(result.contains("Successfully edited"));

        // Verify file was edited correctly
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "Hi World\nGoodbye World");
    }

    #[tokio::test]
    async fn test_edit_file_old_text_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("edit.txt");
        tokio::fs::write(&file_path, "Hello World").await.unwrap();

        let tool = EditFileTool::new(None);
        let args = serde_json::json!({
            "path": file_path.to_str().unwrap(),
            "old_text": "NonExistent",
            "new_text": "Replacement"
        });

        let result = tool.execute(args, &test_ctx()).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("old_text not found"));
    }

    #[tokio::test]
    async fn test_edit_file_multiple_occurrences() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("edit.txt");
        tokio::fs::write(&file_path, "test test test").await.unwrap();

        let tool = EditFileTool::new(None);
        let args = serde_json::json!({
            "path": file_path.to_str().unwrap(),
            "old_text": "test",
            "new_text": "replaced"
        });

        let result = tool.execute(args, &test_ctx()).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("3 times"));
    }

    #[tokio::test]
    async fn test_list_dir_tool() {
        let temp_dir = TempDir::new().unwrap();

        // Create some files and directories
        tokio::fs::write(temp_dir.path().join("file1.txt"), "content1")
            .await
            .unwrap();
        tokio::fs::write(temp_dir.path().join("file2.txt"), "content2")
            .await
            .unwrap();
        tokio::fs::create_dir(temp_dir.path().join("subdir"))
            .await
            .unwrap();

        let tool = ListDirTool::new(None);
        let args = serde_json::json!({
            "path": temp_dir.path().to_str().unwrap()
        });

        let result = tool.execute(args, &test_ctx()).await.unwrap();
        assert!(result.contains("file1.txt"));
        assert!(result.contains("file2.txt"));
        assert!(result.contains("subdir"));
        assert!(result.contains("📁")); // Directory marker
        assert!(result.contains("📄")); // File marker
    }

    #[tokio::test]
    async fn test_list_dir_not_found() {
        let tool = ListDirTool::new(None);
        let args = serde_json::json!({
            "path": "/tmp/nonexistent_dir_12345"
        });

        let result = tool.execute(args, &test_ctx()).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Directory not found"));
    }

    #[tokio::test]
    async fn test_list_dir_empty() {
        let temp_dir = TempDir::new().unwrap();

        let tool = ListDirTool::new(None);
        let args = serde_json::json!({
            "path": temp_dir.path().to_str().unwrap()
        });

        let result = tool.execute(args, &test_ctx()).await.unwrap();
        assert!(result.contains("empty"));
    }

    #[test]
    fn test_tool_schema_generation() {
        let tool = ReadFileTool::new(None);
        let schema = tool.to_schema();

        assert_eq!(schema["type"], "function");
        assert_eq!(schema["function"]["name"], "read_file");
        assert!(schema["function"]["description"].is_string());
        assert!(schema["function"]["parameters"]["properties"]["path"].is_object());
    }
}
