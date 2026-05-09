//! Repo initialization + nested-root discovery.
//!
//! `init` is split-mode: when the workspace folder isn't empty, the frontend
//! re-prompts the user with the entry count before forcing the operation,
//! matching the existing `InitGitRepoResponse::NeedsConfirmation` contract.

use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::cmd::{run_git, run_git_unit};
use crate::errors::GitToolingError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitOutcome {
    Initialized { commit_error: Option<String> },
    AlreadyInitialized,
    NeedsConfirmation { entry_count: u32 },
}

/// Initialize a git repo at `repo` with the given default branch. When the
/// folder already contains files and `force` is false, returns
/// `NeedsConfirmation` so the UI can ask the user before importing them.
pub async fn init(repo: &Path, branch: &str, force: bool) -> Result<InitOutcome, GitToolingError> {
    // Already a repo?
    if repo.join(".git").exists() {
        return Ok(InitOutcome::AlreadyInitialized);
    }

    // Count visible entries (skip dotfiles + .git itself for the count). If
    // any exist and force=false, return needs-confirmation.
    let count = visible_entry_count(repo)?;
    if count > 0 && !force {
        return Ok(InitOutcome::NeedsConfirmation { entry_count: count });
    }

    let branch = branch.trim();
    let branch = if branch.is_empty() { "main" } else { branch };
    run_git_unit(repo, &["init", "-b", branch]).await?;

    // Stage existing files and create an initial commit so subsequent diffs
    // have an anchor. The commit step is best-effort — if the user has no
    // git identity configured, we surface the error string so the UI can
    // explain why the repo is initialized but lacks a commit.
    if count > 0 {
        let _ = run_git_unit(repo, &["add", "-A"]).await;
    }
    let commit_result = crate::commit::commit(repo, "Initial commit").await;
    let commit_error = match commit_result {
        Ok(()) => None,
        Err(e) => Some(e.to_string()),
    };
    Ok(InitOutcome::Initialized { commit_error })
}

/// Walk `root` looking for nested `.git` directories up to `depth` levels
/// deep. Yields the absolute path of each found root. Used by the frontend
/// when the workspace path isn't itself a repo so the user can pick a
/// candidate.
pub fn list_roots(root: &Path, depth: u32) -> Result<Vec<PathBuf>, GitToolingError> {
    let mut roots = Vec::new();
    let depth_usize = depth as usize;
    for entry in WalkDir::new(root)
        .max_depth(depth_usize.max(1))
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            // Skip noisy dirs that never contain user repos.
            let name = e.file_name().to_string_lossy();
            !matches!(name.as_ref(), "node_modules" | "target" | "venv" | ".venv")
        })
    {
        let entry = entry?;
        if entry.file_type().is_dir() && entry.file_name() == ".git" {
            if let Some(parent) = entry.path().parent() {
                roots.push(parent.to_path_buf());
            }
        }
    }
    roots.sort();
    roots.dedup();
    Ok(roots)
}

fn visible_entry_count(root: &Path) -> Result<u32, GitToolingError> {
    if !root.exists() {
        return Ok(0);
    }
    let mut count: u32 = 0;
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == ".git" || name.starts_with(".DS_Store") {
            continue;
        }
        count = count.saturating_add(1);
    }
    Ok(count)
}

/// Verify that `repo` is currently a git repository. Returns `Ok(false)` for
/// "valid path but not a repo" — used as a precondition by other ops.
pub async fn is_repo(repo: &Path) -> Result<bool, GitToolingError> {
    Ok(run_git(repo, &["rev-parse", "--is-inside-work-tree"])
        .await
        .map(|s| s.trim() == "true")
        .unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn init_returns_already_initialized_for_existing_repo() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join(".git")).unwrap();
        let outcome = init(tmp.path(), "main", false).await.unwrap();
        assert_eq!(outcome, InitOutcome::AlreadyInitialized);
    }

    #[tokio::test]
    async fn init_requires_confirmation_for_non_empty_dir() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("README.md"), "hi").unwrap();
        let outcome = init(tmp.path(), "main", false).await.unwrap();
        assert!(matches!(outcome, InitOutcome::NeedsConfirmation { .. }));
    }

    #[tokio::test]
    async fn list_roots_finds_nested_repo() {
        let tmp = TempDir::new().unwrap();
        let nested = tmp.path().join("nested");
        std::fs::create_dir_all(nested.join(".git")).unwrap();
        let roots = list_roots(tmp.path(), 4).unwrap();
        assert!(roots.iter().any(|r| r.ends_with("nested")));
    }
}
