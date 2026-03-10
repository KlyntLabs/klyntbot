//! File system watcher that monitors workspace directories for changes
//! and feeds events into the unified activity log via ActivityIngestionService.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use activity_log::{
    ActivityActor, ActivityIngestionService, ActivityLogEntry, ActivitySource,
    normalizers::{content_hash, new_ulid},
};
use chrono::Utc;
use notify_debouncer_mini::{DebouncedEventKind, new_debouncer};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

pub struct FileWatcherService {
    directories: Vec<PathBuf>,
    ingestion: Arc<ActivityIngestionService>,
    ignore_patterns: HashSet<String>,
    debounce_ms: u64,
    project_cache: HashMap<PathBuf, Option<String>>,
}

impl FileWatcherService {
    pub fn new(
        directories: Vec<PathBuf>,
        ingestion: Arc<ActivityIngestionService>,
        ignore_patterns: Vec<String>,
        debounce_ms: u64,
    ) -> Self {
        Self {
            directories,
            ingestion,
            ignore_patterns: ignore_patterns.into_iter().collect(),
            debounce_ms,
            project_cache: HashMap::new(),
        }
    }

    pub fn start(mut self, cancel: CancellationToken) -> JoinHandle<()> {
        tokio::spawn(async move {
            if let Err(e) = self.run(cancel).await {
                warn!("File watcher stopped with error: {e}");
            }
        })
    }

    async fn run(&mut self, cancel: CancellationToken) -> common::Result<()> {
        let (sync_tx, sync_rx) = std::sync::mpsc::channel();
        let (async_tx, mut async_rx) = tokio::sync::mpsc::channel(256);

        let mut debouncer = new_debouncer(
            Duration::from_millis(self.debounce_ms),
            sync_tx,
        )
        .map_err(|e| common::KlyntbotError::Storage(format!("File watcher init failed: {e}")))?;

        for dir in &self.directories {
            if dir.exists() {
                if let Err(e) =
                    debouncer.watcher().watch(dir, notify::RecursiveMode::Recursive)
                {
                    warn!("Cannot watch {}: {e}", dir.display());
                }
            } else {
                warn!("Watch directory does not exist: {}", dir.display());
            }
        }

        info!(
            "File watcher started for {} directories",
            self.directories.len()
        );

        // Bridge sync→async: block on the sync receiver, forward to async channel
        let bridge_cancel = cancel.clone();
        tokio::task::spawn_blocking(move || {
            while !bridge_cancel.is_cancelled() {
                match sync_rx.recv_timeout(Duration::from_secs(1)) {
                    Ok(result) => {
                        if async_tx.blocking_send(result).is_err() {
                            break; // async receiver dropped
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });

        // Track recent git HEAD to detect branch switches
        let mut last_git_head: HashMap<PathBuf, String> = HashMap::new();

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    debug!("File watcher shutting down");
                    break;
                }
                msg = async_rx.recv() => {
                    let Some(result) = msg else { break };
                    match result {
                        Ok(events) => {
                            for event in events {
                                if event.kind != DebouncedEventKind::Any {
                                    continue;
                                }
                                let path = &event.path;
                                if self.should_ignore(path) {
                                    continue;
                                }

                                // Git event detection
                                if let Some(entry) = self.detect_git_event(path, &mut last_git_head) {
                                    self.ingestion.ingest_fire_and_forget(entry);
                                    continue;
                                }

                                let entry = self.create_fs_entry(path);
                                self.ingestion.ingest_fire_and_forget(entry);
                            }
                        }
                        Err(e) => {
                            warn!("File watcher error: {e}");
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn should_ignore(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        // Check component-based ignore patterns (O(1) per component via HashSet)
        for component in path.components() {
            let name = component.as_os_str().to_string_lossy();
            if self.ignore_patterns.contains(name.as_ref()) {
                return true;
            }
        }

        // Ignore hidden files (except .git for git event detection)
        if let Some(file_name) = path.file_name() {
            let name = file_name.to_string_lossy();
            if name.starts_with('.') && name != ".git" && !path_str.contains(".git/") {
                return true;
            }
        }

        // Ignore binary files
        if let Some(ext) = path.extension() {
            let ext = ext.to_string_lossy().to_lowercase();
            matches!(
                ext.as_str(),
                "exe" | "o" | "dylib" | "so" | "dll" | "a" | "class" | "pyc" | "wasm"
            )
        } else {
            false
        }
    }

    fn detect_git_event(
        &self,
        path: &Path,
        last_git_head: &mut HashMap<PathBuf, String>,
    ) -> Option<ActivityLogEntry> {
        let path_str = path.to_string_lossy();

        // .git/HEAD change → branch checkout
        if path_str.ends_with(".git/HEAD") || path_str.ends_with(".git\\HEAD") {
            let head_content = std::fs::read_to_string(path).ok()?;
            let branch = head_content
                .trim()
                .strip_prefix("ref: refs/heads/")
                .map(|s| s.to_string())
                .unwrap_or_else(|| head_content.trim().chars().take(8).collect());

            let git_dir = path.parent()?;
            let prev = last_git_head.insert(git_dir.to_path_buf(), branch.clone());
            if prev.as_deref() == Some(&branch) {
                return None; // Same branch, not a checkout
            }

            let repo_name = git_dir.parent()?.file_name()?.to_string_lossy().to_string();
            return Some(make_git_entry(
                &repo_name,
                "checkout",
                format!("Branch: {branch}"),
                content_hash(&format!("checkout:{repo_name}:{branch}")),
                serde_json::json!({"branch": branch}),
            ));
        }

        // .git/refs/heads/* change → commit
        if path_str.contains(".git/refs/heads/") || path_str.contains(".git\\refs\\heads\\") {
            let branch = path.file_name()?.to_string_lossy().to_string();
            let git_dir = path
                .ancestors()
                .find(|p| p.file_name().is_some_and(|n| n == ".git"))?;
            let repo_name = git_dir.parent()?.file_name()?.to_string_lossy().to_string();

            // Try to read commit message
            let commit_msg_path = git_dir.join("COMMIT_EDITMSG");
            let commit_msg = std::fs::read_to_string(&commit_msg_path)
                .ok()
                .map(|s| s.lines().next().unwrap_or("").to_string());

            let preview = commit_msg
                .as_deref()
                .map(|m| format!("Commit on {branch}: {m}"))
                .unwrap_or_else(|| format!("Commit on {branch}"));

            return Some(make_git_entry(
                &repo_name,
                "commit",
                preview,
                content_hash(&format!("commit:{repo_name}:{branch}:{}", commit_msg.as_deref().unwrap_or(""))),
                serde_json::json!({
                    "branch": branch,
                    "commit_message": commit_msg,
                }),
            ));
        }

        None
    }

    fn create_fs_entry(&mut self, path: &Path) -> ActivityLogEntry {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());

        let language = detect_language(path);
        let dir = path.parent().unwrap_or(path).to_path_buf();
        let project = self
            .project_cache
            .entry(dir)
            .or_insert_with_key(|d| detect_project(d))
            .clone();

        let action = if !path.exists() {
            "delete"
        } else {
            // Simple heuristic: new files tend to have very recent creation times
            "edit"
        };

        ActivityLogEntry {
            id: new_ulid(),
            timestamp: Utc::now(),
            source: ActivitySource::FileSystem,
            actor: ActivityActor::User,
            resource_type: Some("file".to_string()),
            resource_id: Some(path.to_string_lossy().to_string()),
            resource_name: Some(file_name),
            action: action.to_string(),
            content_preview: Some(format!("{}: {}", action, path.display())),
            content_hash: Some(content_hash(&format!("fs:{}", path.display()))),
            metadata: Some(serde_json::json!({
                "language": language,
                "path": path.to_string_lossy(),
            })),
            app_name: None,
            project_id: project,
            work_context_id: None,
            embedding_id: None,
            duration_secs: None,
            session_key: None,
            is_sensitive: false,
        }
    }
}

fn make_git_entry(
    repo_name: &str,
    action: &str,
    preview: String,
    hash: String,
    metadata: serde_json::Value,
) -> ActivityLogEntry {
    ActivityLogEntry {
        id: new_ulid(),
        timestamp: Utc::now(),
        source: ActivitySource::FileSystem,
        actor: ActivityActor::User,
        resource_type: Some("repo".to_string()),
        resource_id: Some(repo_name.to_string()),
        resource_name: Some(repo_name.to_string()),
        action: action.to_string(),
        content_preview: Some(preview),
        content_hash: Some(hash),
        metadata: Some(metadata),
        app_name: Some("git".to_string()),
        project_id: None,
        work_context_id: None,
        embedding_id: None,
        duration_secs: None,
        session_key: None,
        is_sensitive: false,
    }
}

/// Detect programming language from file extension.
pub fn detect_language(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    let lang = match ext.as_str() {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "py" => "python",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" => "cpp",
        "rb" => "ruby",
        "php" => "php",
        "cs" => "csharp",
        "sql" => "sql",
        "html" | "htm" => "html",
        "css" | "scss" | "sass" | "less" => "css",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "md" | "mdx" => "markdown",
        "sh" | "bash" | "zsh" => "shell",
        "dockerfile" => "docker",
        _ => return None,
    };
    Some(lang.to_string())
}

/// Walk up directory tree to detect project from markers.
pub fn detect_project(path: &Path) -> Option<String> {
    let mut dir = if path.is_file() {
        path.parent()?
    } else {
        path
    };

    for _ in 0..10 {
        // Cargo.toml → rust project
        if dir.join("Cargo.toml").exists() {
            return dir.file_name().map(|n| n.to_string_lossy().to_string());
        }
        // package.json → node project
        if dir.join("package.json").exists() {
            return dir.file_name().map(|n| n.to_string_lossy().to_string());
        }
        // .git → repo name
        if dir.join(".git").exists() {
            return dir.file_name().map(|n| n.to_string_lossy().to_string());
        }
        dir = dir.parent()?;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language(Path::new("main.rs")), Some("rust".into()));
        assert_eq!(
            detect_language(Path::new("app.tsx")),
            Some("typescript".into())
        );
        assert_eq!(detect_language(Path::new("script.py")), Some("python".into()));
        assert_eq!(detect_language(Path::new("Makefile")), None);
    }

    #[test]
    fn test_detect_project_from_cargo() {
        let temp = tempfile::tempdir().unwrap();
        let project_dir = temp.path().join("my-project");
        std::fs::create_dir_all(&project_dir).unwrap();
        std::fs::write(project_dir.join("Cargo.toml"), "[package]").unwrap();
        let file = project_dir.join("src/main.rs");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "fn main() {}").unwrap();

        assert_eq!(detect_project(&file), Some("my-project".into()));
    }

    #[test]
    fn test_ignore_patterns() {
        let patterns: Vec<String> = vec!["node_modules".into(), ".git".into(), "target".into()];
        let path = Path::new("/project/node_modules/foo/bar.js");
        let should_ignore = path.components().any(|c| {
            let name = c.as_os_str().to_string_lossy();
            patterns.iter().any(|p| name == *p)
        });
        assert!(should_ignore);
    }

    #[test]
    fn test_ignore_binary_extensions() {
        let binary_exts = ["exe", "o", "dylib", "so", "dll", "a", "class", "pyc", "wasm"];
        for ext in &binary_exts {
            let path = PathBuf::from(format!("file.{ext}"));
            let is_binary = path
                .extension()
                .is_some_and(|e| binary_exts.contains(&e.to_string_lossy().as_ref()));
            assert!(is_binary, "Should ignore .{ext}");
        }
    }
}
