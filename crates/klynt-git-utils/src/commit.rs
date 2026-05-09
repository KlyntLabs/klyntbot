//! Commit creation. Empty commits are blocked (matches `git commit`'s default
//! behavior). The message is passed via `-F -` (stdin) to avoid arg-list
//! length limits and shell escaping issues.

use std::path::Path;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::cmd::run_git;
use crate::errors::GitToolingError;

pub async fn commit(repo: &Path, message: &str) -> Result<(), GitToolingError> {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return Err(GitToolingError::GitCommand {
            command: "git commit".into(),
            status: std::process::ExitStatus::default(),
            stderr: "commit message must not be empty".into(),
        });
    }
    let mut child = Command::new("git")
        .args(["commit", "-F", "-"])
        .current_dir(repo)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(trimmed.as_bytes()).await?;
        stdin.shutdown().await?;
    }
    let output = child.wait_with_output().await?;
    if !output.status.success() {
        return Err(GitToolingError::GitCommand {
            command: "git commit -F -".into(),
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(())
}

/// Read the staged-vs-HEAD diff and the names of changed files — the input
/// the LLM-backed commit message generator needs. Falls back to the worktree
/// diff when nothing is staged so the user can preview a message before
/// `git add`.
pub async fn collect_commit_context(repo: &Path) -> Result<CommitContext, GitToolingError> {
    let staged = run_git(repo, &["diff", "--cached"]).await?;
    let (diff, source) = if staged.trim().is_empty() {
        let unstaged = run_git(repo, &["diff", "HEAD"]).await.unwrap_or_default();
        (unstaged, CommitDiffSource::Worktree)
    } else {
        (staged, CommitDiffSource::Staged)
    };
    let files = run_git(repo, &["diff", "--cached", "--name-only"]).await?;
    let file_list = if files.trim().is_empty() {
        run_git(repo, &["diff", "HEAD", "--name-only"])
            .await
            .unwrap_or_default()
    } else {
        files
    };
    Ok(CommitContext {
        diff,
        files: file_list
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        source,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitDiffSource {
    Staged,
    Worktree,
}

#[derive(Debug, Clone)]
pub struct CommitContext {
    pub diff: String,
    pub files: Vec<String>,
    pub source: CommitDiffSource,
}
