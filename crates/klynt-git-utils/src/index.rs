//! Index manipulation — stage, unstage, revert. Each op is one `git` call;
//! we deliberately avoid passing user-controlled paths through the shell by
//! using `--`. `git add -A`/`git checkout -- <path>` would also work but
//! `git restore` semantics are clearer in modern git (≥2.23).

use std::path::Path;

use crate::cmd::run_git_unit;
use crate::errors::GitToolingError;

pub async fn stage_file(repo: &Path, path: &str) -> Result<(), GitToolingError> {
    run_git_unit(repo, &["add", "--", path]).await
}

pub async fn stage_all(repo: &Path) -> Result<(), GitToolingError> {
    run_git_unit(repo, &["add", "-A"]).await
}

pub async fn unstage_file(repo: &Path, path: &str) -> Result<(), GitToolingError> {
    // Try modern `git restore --staged` first — fails fast on a no-HEAD repo,
    // in which case the index can only be edited via `rm --cached`. Avoiding
    // the pre-flight `rev-parse --verify HEAD` saves one git spawn per click.
    if let Err(e) = run_git_unit(repo, &["restore", "--staged", "--", path]).await {
        run_git_unit(repo, &["rm", "--cached", "--", path])
            .await
            .map_err(|_| e)?;
    }
    Ok(())
}

/// Discard worktree changes and unstage. Untracked files are deleted (matches
/// the frontend's "discard" wording — user has been confirmed via dialog).
pub async fn revert_file(repo: &Path, path: &str) -> Result<(), GitToolingError> {
    // Drop the staged copy if present, then try to restore working-tree. Both
    // are best-effort; the final fallback (untracked → delete) handles paths
    // git never tracked. We use `ls-files --error-unmatch` as the single
    // tracked-vs-untracked oracle and let it stand in for the HEAD check too.
    let _ = run_git_unit(repo, &["restore", "--staged", "--", path]).await;
    let tracked = crate::cmd::run_git(repo, &["ls-files", "--error-unmatch", "--", path])
        .await
        .is_ok();
    if tracked {
        let _ = run_git_unit(repo, &["restore", "--", path]).await;
    } else {
        let abs = repo.join(path);
        if abs.is_file() || abs.is_symlink() {
            tokio::fs::remove_file(&abs).await.ok();
        }
    }
    Ok(())
}

pub async fn revert_all(repo: &Path) -> Result<(), GitToolingError> {
    // `reset --hard HEAD` fails on a no-HEAD repo; the fallback wipes the
    // index instead. Either way, `clean -fd` strips remaining untracked
    // files (excluding ignored paths) so the worktree matches HEAD.
    if run_git_unit(repo, &["reset", "--hard", "HEAD"])
        .await
        .is_err()
    {
        run_git_unit(repo, &["rm", "-rf", "--cached", "."]).await?;
    }
    run_git_unit(repo, &["clean", "-fd"]).await?;
    Ok(())
}
