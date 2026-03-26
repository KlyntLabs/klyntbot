use crate::types::*;
use ignore::WalkBuilder;
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Directories to always skip (covers non-git dirs where .gitignore doesn't apply).
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    "__pycache__",
    ".cache",
    ".Trash",
    "Library",
];

/// File extensions to skip — compiled artifacts and shared libraries.
const SKIP_EXTENSIONS: &[&str] = &["pyc", "pyo", "class", "o", "obj", "dylib", "so"];

#[derive(Debug, Clone)]
struct FileEntry {
    name: String,
    path: PathBuf,
    /// Which scan_dir this came from — earlier dirs get a mild score boost.
    dir_index: usize,
}

#[derive(Clone)]
pub struct FileSearchSource {
    entries: Arc<RwLock<Vec<FileEntry>>>,
    scan_dirs: Vec<String>,
}

impl FileSearchSource {
    pub fn new(scan_dirs: Vec<String>) -> Self {
        Self {
            entries: Arc::new(RwLock::new(Vec::new())),
            scan_dirs,
        }
    }

    fn walk_dirs(dirs: &[String]) -> Vec<FileEntry> {
        let mut entries = Vec::new();

        for (dir_index, dir) in dirs.iter().enumerate() {
            let expanded = shellexpand::tilde(dir).to_string();
            let root = Path::new(&expanded);
            if !root.exists() {
                continue;
            }

            let walker = WalkBuilder::new(root)
                .hidden(true)
                .git_ignore(true)
                .git_global(true)
                .git_exclude(true)
                .require_git(false)
                .filter_entry(|entry| {
                    // Prune SKIP_DIRS so the walker doesn't descend into them
                    if entry.file_type().is_some_and(|ft| ft.is_dir()) {
                        if let Some(name) = entry.file_name().to_str() {
                            return !SKIP_DIRS.contains(&name);
                        }
                    }
                    true
                })
                .build();

            for result in walker {
                let entry = match result {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::debug!("file_search walk error: {e}");
                        continue;
                    }
                };

                // Skip directories — we only index files
                if entry.file_type().is_none_or(|ft| ft.is_dir()) {
                    continue;
                }

                let path = entry.path();

                let file_name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n,
                    None => continue,
                };

                // Skip unwanted extensions
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if SKIP_EXTENSIONS.contains(&ext) {
                        continue;
                    }
                }

                entries.push(FileEntry {
                    name: file_name.to_string(),
                    path: path.to_path_buf(),
                    dir_index,
                });
            }
        }

        entries
    }

    fn classify_extension(path: &Path) -> FileKind {
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

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        if query.is_empty() {
            return vec![];
        }

        let entries = self.entries.read();
        let scored = super::fuzzy_match(query, &entries, |e| &e.name, limit);

        scored
            .into_iter()
            .map(|(score, e)| {
                let path_str = e.path.display().to_string();
                // Earlier dirs in config get a mild relevance boost, capped at 30%
                let dir_boost = 1.0 - (e.dir_index as f64 * 0.05).min(0.3);
                LauncherItem {
                    id: format!("file:{path_str}"),
                    title: e.name.clone(),
                    subtitle: Some(path_str),
                    icon: Some("file".to_string()),
                    kind: LauncherItemKind::File {
                        path: e.path.clone(),
                        kind: Self::classify_extension(&e.path),
                    },
                    score: (score as f64 / 1000.0) * 0.85 * dir_boost,
                }
            })
            .collect()
    }

    async fn refresh(&self) {
        let dirs = self.scan_dirs.clone();
        let new_entries = tokio::task::spawn_blocking(move || Self::walk_dirs(&dirs))
            .await
            .unwrap_or_else(|e| {
                tracing::error!("file_search walk task failed: {e}");
                vec![]
            });
        tracing::info!("Indexed {} files", new_entries.len());
        *self.entries.write() = new_entries;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::SearchSource;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_index_and_search() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();

        fs::write(base.join("hello.rs"), "fn main() {}").unwrap();
        fs::write(base.join("world.txt"), "hello world").unwrap();
        fs::create_dir_all(base.join("sub")).unwrap();
        fs::write(base.join("sub/nested.rs"), "mod nested;").unwrap();

        fs::create_dir_all(base.join("node_modules")).unwrap();
        fs::write(base.join("node_modules/junk.js"), "junk").unwrap();

        let source = FileSearchSource::new(vec![base.to_string_lossy().to_string()]);
        source.refresh().await;

        let results = source.search("hello", 10).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "hello.rs");

        let results = source.search("rs", 10).await;
        assert!(results.len() >= 2);

        let results = source.search("junk", 10).await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_gitignore_respected() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();

        fs::write(base.join(".gitignore"), "*.log\nbuild/\n").unwrap();
        fs::write(base.join("app.rs"), "fn main() {}").unwrap();
        fs::write(base.join("debug.log"), "log output").unwrap();
        fs::create_dir_all(base.join("build")).unwrap();
        fs::write(base.join("build/output.bin"), "binary").unwrap();

        let source = FileSearchSource::new(vec![base.to_string_lossy().to_string()]);
        source.refresh().await;

        let results = source.search("app", 10).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "app.rs");

        let results = source.search("debug", 10).await;
        assert!(results.is_empty());

        let results = source.search("output", 10).await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_nonexistent_dir_skipped() {
        let source = FileSearchSource::new(vec!["/nonexistent/path/12345".to_string()]);
        source.refresh().await;
        let results = source.search("anything", 10).await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_empty_query_returns_nothing() {
        let source = FileSearchSource::new(vec![]);
        let results = source.search("", 10).await;
        assert!(results.is_empty());
    }
}
