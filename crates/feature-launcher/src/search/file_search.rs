use crate::types::*;
use std::path::PathBuf;

pub struct FileSearchSource;

impl Default for FileSearchSource {
    fn default() -> Self {
        Self::new()
    }
}

impl FileSearchSource {
    pub fn new() -> Self {
        Self
    }

    fn classify_file(path: &std::path::Path) -> FileKind {
        if path.is_dir() {
            return FileKind::Folder;
        }
        match path.extension().and_then(|e| e.to_str()) {
            Some("png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "ico" | "heic") => {
                FileKind::Image
            }
            Some("pdf" | "doc" | "docx" | "txt" | "md" | "rtf" | "pages" | "odt") => {
                FileKind::Document
            }
            Some(
                "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "rb" | "java" | "c" | "cpp"
                | "h" | "swift" | "kt" | "sh" | "toml" | "yaml" | "json" | "html" | "css",
            ) => FileKind::Code,
            Some("zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "dmg") => FileKind::Archive,
            _ => FileKind::File,
        }
    }
}

#[async_trait::async_trait]
impl super::SearchSource for FileSearchSource {
    fn name(&self) -> &'static str {
        "files"
    }

    fn cache_ttl(&self) -> Option<std::time::Duration> {
        Some(std::time::Duration::from_secs(5))
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        if query.is_empty() {
            return vec![];
        }

        #[cfg(target_os = "macos")]
        {
            let output = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                tokio::process::Command::new("mdfind")
                    .args(["-name", query])
                    .output(),
            )
            .await;

            let output = match output {
                Ok(Ok(o)) if o.status.success() => o,
                _ => return vec![],
            };

            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|line| !line.trim().is_empty())
                .take(limit)
                .map(|line| {
                    let path = PathBuf::from(line.trim());
                    let name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let kind = Self::classify_file(&path);
                    LauncherItem {
                        id: format!("file:{}", path.display()),
                        title: name,
                        subtitle: Some(path.display().to_string()),
                        icon: Some("file".to_string()),
                        kind: LauncherItemKind::File { path, kind },
                        score: 0.8,
                    }
                })
                .collect()
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = (query, limit);
            vec![]
        }
    }
}
