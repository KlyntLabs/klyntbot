use crate::types::*;
use ignore::{overrides::OverrideBuilder, WalkBuilder};
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Directories to skip during walk (applies to non-git-tracked directories).
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
    /// Filename (used for fuzzy matching).
    name: String,
    /// Full path to the file or directory.
    path: PathBuf,
    /// Index of the scan directory this entry came from (for `dir_boost`).
    dir_index: usize,
    /// Classified kind of the file.
    kind: FileKind,
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

    /// Walk configured directories with `ignore`-aware traversal and collect file entries.
    fn walk_dirs(dirs: &[String]) -> Vec<FileEntry> {
        let mut entries = Vec::new();

        for (dir_index, dir) in dirs.iter().enumerate() {
            let expanded = shellexpand::tilde(dir).to_string();
            let root = Path::new(&expanded);
            if !root.exists() {
                continue;
            }

            // Build overrides to prevent descending into skip dirs
            let mut overrides = OverrideBuilder::new(root);
            for skip in SKIP_DIRS {
                // Negated glob: ignore everything inside these directories
                let _ = overrides.add(&format!("!{skip}"));
                let _ = overrides.add(&format!("!{skip}/**"));
            }
            let overrides = overrides.build().ok();

            let mut builder = WalkBuilder::new(root);
            builder
                .hidden(true)
                .git_ignore(true)
                .git_global(true)
                .git_exclude(true)
                .require_git(false);
            if let Some(ov) = overrides {
                builder.overrides(ov);
            }
            let walker = builder.build();

            for result in walker {
                let entry = match result {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::debug!("file_search walk error: {}", e);
                        continue;
                    }
                };

                let path = entry.path();

                // Skip the root directory itself
                if path == root {
                    continue;
                }

                let file_name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n,
                    None => continue,
                };

                // Skip .DS_Store
                if file_name == ".DS_Store" {
                    continue;
                }

                let is_dir = entry.file_type().is_some_and(|ft| ft.is_dir());

                // Skip unwanted extensions
                if !is_dir {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        if SKIP_EXTENSIONS.contains(&ext) {
                            continue;
                        }
                    }
                }

                let kind = Self::classify_extension(path, is_dir);

                entries.push(FileEntry {
                    name: file_name.to_string(),
                    path: path.to_path_buf(),
                    dir_index,
                    kind,
                });
            }
        }

        entries
    }

    fn classify_extension(path: &Path, is_dir: bool) -> FileKind {
        if is_dir {
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

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        if query.is_empty() {
            return vec![];
        }

        let entries = self.entries.read();
        let scored = super::fuzzy_match(query, &entries, |e| &e.name, limit);

        scored
            .into_iter()
            .map(|(score, e)| {
                let dir_boost = 1.0 - (e.dir_index as f64 * 0.05).min(0.3);
                LauncherItem {
                    id: format!("file:{}", e.path.display()),
                    title: e.name.clone(),
                    subtitle: Some(e.path.display().to_string()),
                    icon: Some("file".to_string()),
                    kind: LauncherItemKind::File {
                        path: e.path.clone(),
                        kind: e.kind.clone(),
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
            .unwrap_or_default();
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
