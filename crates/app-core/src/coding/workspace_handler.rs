use crate::AppCore;
use common::Result;

/// Result for workspace_meta_read.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaReadResult {
    pub exists: bool,
    pub content: Option<String>,
    pub truncated: bool,
}

/// Result for workspace_file_read.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileReadResult {
    pub content: String,
    pub truncated: bool,
    pub mime: String,
    pub encoding: String,
}

const MAX_META_BYTES: usize = 100 * 1024; // 100 KB
const MAX_FILE_BYTES: usize = 1024 * 1024; // 1 MB

impl AppCore {
    /// Read workspace metadata files (AGENTS.md, config.json, etc.).
    ///
    /// `scope` is "workspace" or "global".
    /// `kind` is "agents" or "config".
    #[tracing::instrument(skip(self), err)]
    pub async fn workspace_meta_read(
        &self,
        workspace_id: &str,
        scope: &str,
        kind: &str,
    ) -> Result<MetaReadResult> {
        let path = match (scope, kind) {
            ("workspace", "agents") => {
                let ws = self
                    .repos
                    .workspaces
                    .get(workspace_id)
                    .await
                    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
                std::path::PathBuf::from(&ws.path).join("AGENTS.md")
            }
            ("workspace", "config") => {
                let ws = self
                    .repos
                    .workspaces
                    .get(workspace_id)
                    .await
                    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
                std::path::PathBuf::from(&ws.path)
                    .join(".klyntbot")
                    .join("config.json")
            }
            ("global", "agents") => {
                let config = self.config.read().await;
                config.data_dir_path().join("AGENTS.md")
            }
            ("global", "config") => {
                let config = self.config.read().await;
                config.data_dir_path().join("config.json")
            }
            _ => {
                return Err(common::KlyntbotError::Storage(format!(
                    "unknown scope/kind: {scope}/{kind}"
                )));
            }
        };

        if !path.exists() {
            return Ok(MetaReadResult {
                exists: false,
                content: None,
                truncated: false,
            });
        }

        let raw = std::fs::read_to_string(&path).map_err(|e| {
            common::KlyntbotError::Storage(format!("failed to read {}: {e}", path.display()))
        })?;

        let truncated = raw.len() > MAX_META_BYTES;
        let content = if truncated {
            raw[..MAX_META_BYTES].to_string()
        } else {
            raw
        };

        Ok(MetaReadResult {
            exists: true,
            content: Some(content),
            truncated,
        })
    }

    /// Write workspace metadata files.
    #[tracing::instrument(skip(self, content), err)]
    pub async fn workspace_meta_write(
        &self,
        workspace_id: &str,
        scope: &str,
        kind: &str,
        content: &str,
    ) -> Result<()> {
        let path = match (scope, kind) {
            ("workspace", "agents") => {
                let ws = self
                    .repos
                    .workspaces
                    .get(workspace_id)
                    .await
                    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
                std::path::PathBuf::from(&ws.path).join("AGENTS.md")
            }
            ("workspace", "config") => {
                let ws = self
                    .repos
                    .workspaces
                    .get(workspace_id)
                    .await
                    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
                std::path::PathBuf::from(&ws.path)
                    .join(".klyntbot")
                    .join("config.json")
            }
            ("global", "agents") => {
                let config = self.config.read().await;
                config.data_dir_path().join("AGENTS.md")
            }
            ("global", "config") => {
                let config = self.config.read().await;
                config.data_dir_path().join("config.json")
            }
            _ => {
                return Err(common::KlyntbotError::Storage(format!(
                    "unknown scope/kind: {scope}/{kind}"
                )));
            }
        };

        // Create parent dirs if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                common::KlyntbotError::Storage(format!(
                    "failed to create dir {}: {e}",
                    parent.display()
                ))
            })?;
        }

        std::fs::write(&path, content).map_err(|e| {
            common::KlyntbotError::Storage(format!("failed to write {}: {e}", path.display()))
        })?;

        Ok(())
    }

    /// Read a file from within the workspace directory.
    /// Validates that the path doesn't escape the workspace.
    #[tracing::instrument(skip(self), err)]
    pub async fn workspace_file_read(
        &self,
        workspace_id: &str,
        rel_path: &str,
    ) -> Result<FileReadResult> {
        let ws = self
            .repos
            .workspaces
            .get(workspace_id)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        let ws_root = std::path::Path::new(&ws.path);
        let full_path = ws_root.join(rel_path);

        // Canonicalize and validate path doesn't escape workspace
        let canonical = full_path
            .canonicalize()
            .map_err(|e| common::KlyntbotError::Storage(format!("invalid path: {e}")))?;
        let ws_canonical = ws_root
            .canonicalize()
            .map_err(|e| common::KlyntbotError::Storage(format!("invalid workspace path: {e}")))?;
        if !canonical.starts_with(&ws_canonical) {
            return Err(common::KlyntbotError::Storage(
                "path escapes workspace boundary".into(),
            ));
        }

        let raw = std::fs::read_to_string(&canonical).map_err(|e| {
            common::KlyntbotError::Storage(format!("failed to read {}: {e}", canonical.display()))
        })?;

        let truncated = raw.len() > MAX_FILE_BYTES;
        let content = if truncated {
            raw[..MAX_FILE_BYTES].to_string()
        } else {
            raw
        };

        let mime = mime_guess(&canonical);
        let encoding = "utf-8".to_string();

        Ok(FileReadResult {
            content,
            truncated,
            mime,
            encoding,
        })
    }

    /// List files in the workspace with optional fuzzy query.
    #[tracing::instrument(skip(self), err)]
    pub async fn workspace_files_list(
        &self,
        workspace_id: &str,
        query: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<String>> {
        let ws = self
            .repos
            .workspaces
            .get(workspace_id)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        let ws_root = std::path::Path::new(&ws.path);
        let max = limit.unwrap_or(50).min(200);

        let mut files: Vec<String> = Vec::new();
        let walker = walkdir::WalkDir::new(ws_root)
            .max_depth(10)
            .follow_links(false)
            .into_iter();

        for entry in walker.filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && name != "node_modules" && name != "target"
        }) {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(ws_root)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .into_owned();
            files.push(rel);
        }

        if let Some(q) = query {
            if !q.is_empty() {
                // Simple substring filter (nucleo integration would be better)
                let q_lower = q.to_lowercase();
                files.retain(|f| f.to_lowercase().contains(&q_lower));
            }
        }

        files.sort();
        files.truncate(max);
        Ok(files)
    }

    /// Write content to an absolute path (for save-dialog flows with user consent).
    #[tracing::instrument(skip(self, content), err)]
    pub async fn text_file_write(&self, path: &str, content: &str) -> Result<()> {
        let p = std::path::Path::new(path);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                common::KlyntbotError::Storage(format!(
                    "failed to create dir {}: {e}",
                    parent.display()
                ))
            })?;
        }
        std::fs::write(p, content)
            .map_err(|e| common::KlyntbotError::Storage(format!("failed to write {path}: {e}")))
    }

    /// Read an image file and return a data URL.
    #[tracing::instrument(skip(self), err)]
    pub async fn image_data_url(&self, path: &str) -> Result<String> {
        let p = std::path::Path::new(path);
        let bytes = std::fs::read(p).map_err(|e| {
            common::KlyntbotError::Storage(format!("failed to read image {path}: {e}"))
        })?;
        let mime = match p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str()
        {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            _ => "application/octet-stream",
        };
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        Ok(format!("data:{mime};base64,{b64}"))
    }
}

fn mime_guess(path: &std::path::Path) -> String {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "md" => "text/markdown".into(),
        "json" => "application/json".into(),
        "toml" => "application/toml".into(),
        "yaml" | "yml" => "application/yaml".into(),
        "rs" => "text/x-rust".into(),
        "ts" | "tsx" => "text/typescript".into(),
        "js" | "jsx" => "text/javascript".into(),
        "py" => "text/x-python".into(),
        "html" => "text/html".into(),
        "css" => "text/css".into(),
        "svg" => "image/svg+xml".into(),
        "png" => "image/png".into(),
        "jpg" | "jpeg" => "image/jpeg".into(),
        "gif" => "image/gif".into(),
        "webp" => "image/webp".into(),
        "txt" => "text/plain".into(),
        _ => "application/octet-stream".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_read_result_serializes() {
        let r = MetaReadResult {
            exists: true,
            content: Some("hello".into()),
            truncated: false,
        };
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["exists"], true);
        assert_eq!(json["content"], "hello");
    }

    #[test]
    fn file_read_result_serializes() {
        let r = FileReadResult {
            content: "test".into(),
            truncated: false,
            mime: "text/plain".into(),
            encoding: "utf-8".into(),
        };
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["mime"], "text/plain");
    }

    #[test]
    fn mime_guess_identifies_common_types() {
        assert_eq!(mime_guess(std::path::Path::new("foo.rs")), "text/x-rust");
        assert_eq!(
            mime_guess(std::path::Path::new("foo.json")),
            "application/json"
        );
        assert_eq!(mime_guess(std::path::Path::new("foo.png")), "image/png");
    }
}
