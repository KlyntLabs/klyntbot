use std::path::{Path, PathBuf};
use tokio::process::Command;

use crate::errors::GitToolingError;

/// Returns true if `path` is inside a git working tree.
pub async fn is_inside_git_repo(path: &Path) -> Result<bool, GitToolingError> {
    let out = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(path)
        .output()
        .await?;
    Ok(out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "true")
}

/// Returns the absolute path of the git working-tree root containing `path`.
pub async fn get_git_repo_root(path: &Path) -> Result<PathBuf, GitToolingError> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(path)
        .output()
        .await?;
    if !out.status.success() {
        return Err(GitToolingError::NotAGitRepository {
            path: path.to_path_buf(),
        });
    }
    let s = String::from_utf8(out.stdout).map_err(|e| GitToolingError::GitOutputUtf8 {
        command: "git rev-parse --show-toplevel".into(),
        source: e,
    })?;
    Ok(PathBuf::from(s.trim()))
}
