//! Resolve a cwd to a canonical `RepoScope`.
//!
//! Strategy:
//! 1. `git rev-parse --show-toplevel` → worktree root.
//! 2. `git config --get remote.origin.url` → canonical id when present.
//! 3. Fall back to `local:<sanitized-worktree-basename>`.
//!
//! Result cached per-cwd in a process-wide `RwLock<HashMap>`.

use crate::scope::RepoScope;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::RwLock;

static CACHE: once_cell::sync::Lazy<RwLock<std::collections::HashMap<PathBuf, Option<RepoScope>>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(std::collections::HashMap::new()));

/// Resolve `cwd` → `RepoScope`. Returns `None` only if `cwd` doesn't exist.
#[must_use]
pub fn resolve_scope(cwd: &Path) -> Option<RepoScope> {
    let key = std::fs::canonicalize(cwd).ok()?;
    if let Some(hit) = CACHE.read().ok().and_then(|m| m.get(&key).cloned()) {
        return hit;
    }
    let scope = compute(&key);
    if let Ok(mut m) = CACHE.write() {
        m.insert(key, scope.clone());
    }
    scope
}

fn compute(cwd: &Path) -> Option<RepoScope> {
    // Fall-back identity uses the basename.
    let basename = cwd
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    let fallback = RepoScope {
        repo_id: format!("local:{}", sanitize(basename)),
        root: cwd.to_path_buf(),
        git_hash: None,
        branch: None,
    };

    let Some(root) = run(cwd, &["rev-parse", "--show-toplevel"])
        .and_then(|s| std::fs::canonicalize(s.trim()).ok())
    else {
        return Some(fallback);
    };

    let repo_id = run(cwd, &["config", "--get", "remote.origin.url"])
        .and_then(|s| canonicalize_remote(s.trim()))
        .unwrap_or_else(|| {
            format!(
                "local:{}",
                sanitize(
                    root.file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                )
            )
        });
    let git_hash = run(cwd, &["rev-parse", "HEAD"]).map(|s| s.trim().to_string());
    let branch = run(cwd, &["rev-parse", "--abbrev-ref", "HEAD"])
        .map(|s| s.trim().to_string())
        .filter(|s| s != "HEAD");

    Some(RepoScope {
        repo_id,
        root,
        git_hash,
        branch,
    })
}

fn run(cwd: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

fn canonicalize_remote(raw: &str) -> Option<String> {
    // `git@github.com:org/repo.git` → `github.com/org/repo`
    if let Some(rest) = raw.strip_prefix("git@") {
        if let Some((host, path)) = rest.split_once(':') {
            return Some(format!("{host}/{}", strip_git_suffix(path)));
        }
    }
    // `https://github.com/org/repo.git` → `github.com/org/repo`
    if let Ok(url) = url::Url::parse(raw) {
        let host = url.host_str()?;
        let path = url.path().trim_start_matches('/');
        return Some(format!("{host}/{}", strip_git_suffix(path)));
    }
    None
}

fn strip_git_suffix(path: &str) -> String {
    path.trim_end_matches('/')
        .trim_end_matches(".git")
        .to_string()
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
