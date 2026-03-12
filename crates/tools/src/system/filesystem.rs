//! Filesystem tools: read, write, edit, list.

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::debug;

use common::{Result, ToolError};
use tools_core::{RoutingContext, ToolParams};

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
pub struct FsToolBase {
    allowed_dir: Option<PathBuf>,
}

impl FsToolBase {
    pub fn new(allowed_dir: Option<PathBuf>) -> Self {
        Self { allowed_dir }
    }

    pub fn resolve_path(&self, path: &str) -> Result<PathBuf> {
        resolve_path(path, self.allowed_dir.as_ref())
    }

    pub fn allowed_dir(&self) -> &Option<PathBuf> {
        &self.allowed_dir
    }

    /// Resolve the search root: explicit path > allowed_dir > cwd.
    /// Shared by GrepTool and GlobTool.
    pub fn resolve_search_root(&self, explicit_path: Option<&str>) -> Result<PathBuf> {
        if let Some(path) = explicit_path {
            self.resolve_path(path)
        } else if let Some(dir) = self.allowed_dir.as_ref() {
            Ok(dir.clone())
        } else {
            std::env::current_dir().map_err(|e| ToolError::ExecutionFailed(e.to_string()).into())
        }
    }
}

#[derive(Debug, ToolParams)]
pub struct ReadFileParams {
    /// The file path to read
    #[param(required)]
    pub path: String,
}

/// Tool to read file contents
#[derive(tools_core::Tool)]
#[tool(
    name = "read_file",
    description = "Read the contents of a file at the given path.",
    params = "ReadFileParams",
    permission = "read_only",
    category = "FileSystem",
    tags = "file,read,content",
    cost = "Free"
)]
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
impl tools_core::ToolExecute for ReadFileTool {
    type Params = ReadFileParams;

    async fn execute(&self, params: ReadFileParams, _ctx: &RoutingContext) -> Result<String> {
        let path = &params.path;

        debug!("Reading file: {}", path);

        let file_path = self.base.resolve_path(path)?;

        match fs::read_to_string(&file_path).await {
            Ok(content) => Ok(content),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(ToolError::ExecutionFailed(format!("File not found: {}", path)).into())
            }
            Err(e) if e.raw_os_error() == Some(21 /* EISDIR */) => {
                Err(ToolError::ExecutionFailed(format!("Not a file: {}", path)).into())
            }
            Err(e) => Err(ToolError::ExecutionFailed(format!("Error reading file: {}", e)).into()),
        }
    }
}

#[derive(Debug, ToolParams)]
pub struct WriteFileParams {
    /// The file path to write to
    #[param(required)]
    pub path: String,

    /// The content to write
    #[param(required)]
    pub content: String,
}

/// Tool to write content to a file
#[derive(tools_core::Tool)]
#[tool(
    name = "write_file",
    description = "Write content to a file at the given path. Creates parent directories if needed.",
    params = "WriteFileParams",
    permission = "elevated",
    category = "FileSystem",
    tags = "file,write,create",
    cost = "Free"
)]
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
impl tools_core::ToolExecute for WriteFileTool {
    type Params = WriteFileParams;

    async fn execute(&self, params: WriteFileParams, _ctx: &RoutingContext) -> Result<String> {
        let path = &params.path;
        let content = &params.content;

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

#[derive(Debug, ToolParams)]
pub struct EditFileParams {
    /// The file path to edit
    #[param(required)]
    pub path: String,

    /// The exact text to find and replace
    #[param(required)]
    pub old_text: String,

    /// The text to replace with
    #[param(required)]
    pub new_text: String,
}

/// Tool to edit a file by replacing text
#[derive(tools_core::Tool)]
#[tool(
    name = "edit_file",
    description = "Edit a file by replacing old_text with new_text. The old_text must exist exactly in the file.",
    params = "EditFileParams",
    permission = "elevated",
    category = "FileSystem",
    tags = "file,edit,modify",
    cost = "Free"
)]
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
impl tools_core::ToolExecute for EditFileTool {
    type Params = EditFileParams;

    async fn execute(&self, params: EditFileParams, _ctx: &RoutingContext) -> Result<String> {
        let path = &params.path;
        let old_text = &params.old_text;
        let new_text = &params.new_text;

        debug!("Editing file: {}", path);

        let file_path = self.base.resolve_path(path)?;

        let content = match fs::read_to_string(&file_path).await {
            Ok(content) => content,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(ToolError::ExecutionFailed(format!("File not found: {}", path)).into());
            }
            Err(e) if e.raw_os_error() == Some(21 /* EISDIR */) => {
                return Err(ToolError::ExecutionFailed(format!("Not a file: {}", path)).into());
            }
            Err(e) => {
                return Err(
                    ToolError::ExecutionFailed(format!("Failed to read file: {}", e)).into(),
                );
            }
        };

        if !content.contains(old_text.as_str()) {
            return Err(ToolError::ExecutionFailed(
                "old_text not found in file. Make sure it matches exactly.".to_string(),
            )
            .into());
        }

        let count = content.matches(old_text.as_str()).count();
        if count > 1 {
            return Err(ToolError::ExecutionFailed(format!(
                "old_text appears {} times. Please provide more context to make it unique.",
                count
            ))
            .into());
        }

        let new_content = content.replacen(old_text.as_str(), new_text, 1);

        fs::write(&file_path, new_content)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write file: {}", e)))?;

        Ok(format!("Successfully edited {}", path))
    }
}

#[derive(Debug, ToolParams)]
pub struct ListDirParams {
    /// The directory path to list
    #[param(required)]
    pub path: String,
}

/// Tool to list directory contents
#[derive(tools_core::Tool)]
#[tool(
    name = "list_dir",
    description = "List the contents of a directory.",
    params = "ListDirParams",
    permission = "read_only",
    category = "FileSystem",
    tags = "file,directory,list",
    cost = "Free"
)]
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
impl tools_core::ToolExecute for ListDirTool {
    type Params = ListDirParams;

    async fn execute(&self, params: ListDirParams, _ctx: &RoutingContext) -> Result<String> {
        let path = &params.path;

        debug!("Listing directory: {}", path);

        let dir_path = self.base.resolve_path(path)?;

        let mut entries = match fs::read_dir(&dir_path).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(
                    ToolError::ExecutionFailed(format!("Directory not found: {}", path)).into(),
                );
            }
            Err(e) if e.raw_os_error() == Some(20 /* ENOTDIR */) => {
                return Err(
                    ToolError::ExecutionFailed(format!("Not a directory: {}", path)).into(),
                );
            }
            Err(e) => {
                return Err(
                    ToolError::ExecutionFailed(format!("Failed to read directory: {}", e)).into(),
                );
            }
        };

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

/// Register read-only filesystem tools (ReadFile + ListDir only).
/// Used by Research and Analyst sub-agent profiles.
pub fn register_fs_read_tools(
    registry: &mut crate::registry::ToolRegistry,
    allowed_dir: Option<PathBuf>,
) {
    registry.register(ReadFileTool::new(allowed_dir.clone()));
    registry.register(ListDirTool::new(allowed_dir));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RoutingContext, Tool};
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
        tokio::fs::write(&file_path, "Allowed content")
            .await
            .unwrap();

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
        tokio::fs::write(&file_path, "test test test")
            .await
            .unwrap();

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

    #[test]
    fn test_register_fs_read_tools_only_registers_read_and_list() {
        let mut registry = crate::registry::ToolRegistry::new();
        register_fs_read_tools(&mut registry, None);
        let defs = registry.get_definitions();
        let names: Vec<&str> = defs
            .iter()
            .filter_map(|d| d["function"]["name"].as_str())
            .collect();
        assert!(
            names.contains(&"read_file"),
            "Expected read_file in {:?}",
            names
        );
        assert!(
            names.contains(&"list_dir"),
            "Expected list_dir in {:?}",
            names
        );
        assert!(
            !names.contains(&"write_file"),
            "write_file should NOT be registered"
        );
        assert!(
            !names.contains(&"edit_file"),
            "edit_file should NOT be registered"
        );
    }
}
