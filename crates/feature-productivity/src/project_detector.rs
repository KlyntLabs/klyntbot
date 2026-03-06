//! ProjectDetector — resolves the active project from terminal CWD,
//! IDE window titles, and browser URL patterns.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::repos::ProjectRepo;
use crate::tracker::categorizer::Categorizer;
use crate::types::ProductivityProject;

/// Known terminal app names (lowercased).
const TERMINAL_APPS: &[&str] = &[
    "ghostty", "iterm2", "terminal", "warp",
    "alacritty", "kitty", "hyper",
];

/// Known terminal bundle ID prefixes.
const TERMINAL_BUNDLE_PREFIXES: &[&str] = &[
    "com.mitchellh.ghostty",
    "com.googlecode.iterm2",
    "com.apple.terminal",
    "dev.warp.warp-stable",
    "io.alacritty",
    "net.kovidgoyal.kitty",
];

/// Known IDE app names (lowercased).
const IDE_APPS: &[&str] = &[
    "visual studio code", "code", "cursor", "zed",
    "xcode", "intellij idea", "webstorm", "goland",
    "rustrover", "pycharm", "phpstorm",
];

/// IDE bundle prefixes for matching.
const IDE_BUNDLE_PREFIXES: &[&str] = &[
    "com.microsoft.vscode",
    "com.todesktop.230313mzl4w4u92", // Cursor
    "dev.zed.zed",
    "com.apple.dt.xcode",
    "com.jetbrains.",
];

/// IDE browser suffix patterns to strip from window titles.
const IDE_SUFFIXES: &[&str] = &[
    " - visual studio code",
    " — visual studio code",
    " - cursor",
    " — cursor",
    " - zed",
    " — zed",
    " — xcode",
];

/// Maximum number of entries in the git root cache before eviction.
const GIT_ROOT_CACHE_MAX: usize = 512;

pub struct ProjectDetector {
    /// Cached projects from DB, keyed by path.
    projects_by_path: Arc<RwLock<HashMap<String, ProductivityProject>>>,
    /// Cached projects by URL pattern for fast matching.
    url_pattern_map: Arc<RwLock<Vec<(String, String)>>>,  // (pattern, project_id)
    /// Git root cache to avoid repeated filesystem walks.
    git_root_cache: Arc<RwLock<HashMap<PathBuf, Option<PathBuf>>>>,
}

impl Default for ProjectDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectDetector {
    pub fn new() -> Self {
        Self {
            projects_by_path: Arc::new(RwLock::new(HashMap::new())),
            url_pattern_map: Arc::new(RwLock::new(Vec::new())),
            git_root_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Reload project list from DB.
    pub async fn refresh(&self, repo: &ProjectRepo) -> common::Result<()> {
        let projects = repo.list_all().await?;
        let mut by_path = self.projects_by_path.write().await;
        let mut url_map = self.url_pattern_map.write().await;
        by_path.clear();
        url_map.clear();
        for p in projects {
            for pattern in &p.url_patterns {
                url_map.push((pattern.clone(), p.id.clone()));
            }
            by_path.insert(p.path.clone(), p);
        }
        Ok(())
    }

    /// Detect project for the current frontmost app.
    /// Returns (project_id, should_auto_register_path).
    pub async fn detect(
        &self,
        app_name: &str,
        bundle_id: Option<&str>,
        window_title: Option<&str>,
        url: Option<&str>,
        pid: i32,
    ) -> Option<DetectionResult> {
        let name_lower = app_name.to_lowercase();
        let bid_lower = bundle_id.map(|b| b.to_lowercase());

        // 1. Terminal → CWD → git root
        if matches_app_type(&name_lower, bid_lower.as_deref(), TERMINAL_APPS, TERMINAL_BUNDLE_PREFIXES) {
            let cwd = {
                let p = pid;
                tokio::task::spawn_blocking(move || get_terminal_cwd(p))
                    .await
                    .ok()
                    .flatten()
            };
            if let Some(cwd) = cwd {
                if let Some(git_root) = self.find_git_root_cached(&cwd).await {
                    let path_str = git_root.to_string_lossy().to_string();
                    let by_path = self.projects_by_path.read().await;
                    if let Some(proj) = by_path.get(&path_str) {
                        return Some(DetectionResult::Known(proj.id.clone()));
                    }
                    drop(by_path);
                    let dirname = git_root
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    return Some(DetectionResult::AutoRegister {
                        path: path_str,
                        display_name: dirname,
                    });
                }
            }
        }

        // 2. IDE → parse project from window title
        if matches_app_type(&name_lower, bid_lower.as_deref(), IDE_APPS, IDE_BUNDLE_PREFIXES) {
            if let Some(title) = window_title {
                if let Some(project_name) = extract_ide_project(title) {
                    let by_path = self.projects_by_path.read().await;
                    for proj in by_path.values() {
                        let basename = Path::new(&proj.path)
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string());
                        if basename.as_deref() == Some(&project_name) {
                            return Some(DetectionResult::Known(proj.id.clone()));
                        }
                    }
                }
            }
        }

        // 3. Browser → URL pattern matching
        if Categorizer::is_browser(app_name, bundle_id) {
            if let Some(u) = url {
                let url_map = self.url_pattern_map.read().await;
                for (pattern, project_id) in url_map.iter() {
                    if u.contains(pattern) {
                        return Some(DetectionResult::Known(project_id.clone()));
                    }
                }
            }
        }

        None
    }

    async fn find_git_root_cached(&self, cwd: &Path) -> Option<PathBuf> {
        // Single write lock with double-check to avoid TOCTOU race
        let mut cache = self.git_root_cache.write().await;
        if let Some(cached) = cache.get(cwd) {
            return cached.clone();
        }
        let result = find_git_root(cwd);
        // Evict oldest entries if cache is full
        if cache.len() >= GIT_ROOT_CACHE_MAX {
            let keys: Vec<_> = cache.keys().take(cache.len() / 4).cloned().collect();
            for k in keys {
                cache.remove(&k);
            }
        }
        cache.insert(cwd.to_path_buf(), result.clone());
        result
    }
}

/// Check if an app matches a known type by name or bundle ID prefix.
fn matches_app_type(
    name_lower: &str,
    bid_lower: Option<&str>,
    app_names: &[&str],
    bundle_prefixes: &[&str],
) -> bool {
    if app_names.contains(&name_lower) {
        return true;
    }
    if let Some(bid) = bid_lower {
        if bundle_prefixes.iter().any(|p| bid.starts_with(p)) {
            return true;
        }
    }
    false
}

#[derive(Debug)]
pub enum DetectionResult {
    Known(String),
    AutoRegister { path: String, display_name: String },
}

/// Walk up from `path` to find the nearest `.git` directory.
fn find_git_root(path: &Path) -> Option<PathBuf> {
    let mut current = path;
    loop {
        if current.join(".git").exists() {
            return Some(current.to_path_buf());
        }
        current = current.parent()?;
    }
}

/// Get the CWD of the foreground shell process inside a terminal.
///
/// Strategy: find the deepest child process of the terminal PID,
/// then read its CWD via `lsof`.
#[cfg(target_os = "macos")]
fn get_terminal_cwd(terminal_pid: i32) -> Option<PathBuf> {
    use std::process::Command;

    // Find shell child processes
    let output = Command::new("lsof")
        .args(["-a", "-p", &terminal_pid.to_string(), "-d", "cwd", "-Fn"])
        .output()
        .ok()?;

    if !output.status.success() {
        // Try finding child processes first
        return get_child_cwd(terminal_pid);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix('n') {
            let p = PathBuf::from(path);
            if p.exists() {
                return Some(p);
            }
        }
    }

    // Fallback: find child shell process CWD
    get_child_cwd(terminal_pid)
}

#[cfg(target_os = "macos")]
fn get_child_cwd(parent_pid: i32) -> Option<PathBuf> {
    use std::process::Command;

    // Use pgrep to find child processes
    let output = Command::new("pgrep")
        .args(["-P", &parent_pid.to_string()])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Try each child, recursively looking for the deepest one
    for line in stdout.lines() {
        let child_pid: i32 = line.trim().parse().ok()?;
        // Try grandchildren first (for tmux/shell nesting)
        if let Some(cwd) = get_child_cwd(child_pid) {
            return Some(cwd);
        }
        // Then try this child's CWD
        let lsof = Command::new("lsof")
            .args(["-a", "-p", &child_pid.to_string(), "-d", "cwd", "-Fn"])
            .output()
            .ok()?;
        if lsof.status.success() {
            let out = String::from_utf8_lossy(&lsof.stdout);
            for l in out.lines() {
                if let Some(path) = l.strip_prefix('n') {
                    let p = PathBuf::from(path);
                    // Skip home directory (not a meaningful project path)
                    if p.exists() && p != dirs::home_dir().unwrap_or_default() {
                        return Some(p);
                    }
                }
            }
        }
    }
    None
}

#[cfg(not(target_os = "macos"))]
fn get_terminal_cwd(_terminal_pid: i32) -> Option<PathBuf> {
    None
}

/// Extract project name from IDE window title.
/// "main.rs — klyntbot — Cursor" → "klyntbot"
/// "lib.rs — MailGate" → "MailGate"
fn extract_ide_project(title: &str) -> Option<String> {
    let mut cleaned = title.to_string();
    // Strip IDE suffix
    let lower = cleaned.to_lowercase();
    for suffix in IDE_SUFFIXES {
        if let Some(pos) = lower.rfind(suffix) {
            cleaned = cleaned[..pos].to_string();
            break;
        }
    }
    // The project name is typically the last segment before the IDE name
    // "main.rs — klyntbot" → split on " — " or " - ", take last
    for sep in &[" — ", " - "] {
        if let Some(pos) = cleaned.rfind(sep) {
            let segment = cleaned[pos + sep.len()..].trim();
            if !segment.is_empty() && !segment.contains('.') {
                return Some(segment.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_git_root() {
        // This test only works if run inside a git repo
        let cwd = std::env::current_dir().unwrap();
        let root = find_git_root(&cwd);
        assert!(root.is_some());
        assert!(root.unwrap().join(".git").exists());
    }

    #[test]
    fn test_extract_ide_project() {
        assert_eq!(
            extract_ide_project("main.rs — klyntbot — Cursor"),
            Some("klyntbot".to_string())
        );
        assert_eq!(
            extract_ide_project("lib.rs — MailGate - Visual Studio Code"),
            Some("MailGate".to_string())
        );
        assert_eq!(
            extract_ide_project("Welcome — Zed"),
            None // "Welcome" has no project context
        );
    }
}
