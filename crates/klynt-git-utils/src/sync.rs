//! Network ops — push, pull, fetch, sync (= pull then push). Each shells out
//! to the user's `git` so credentials, SSH agent, GPG signing, and HTTPS
//! cred helpers all behave exactly as on the CLI. We do *not* configure
//! any auth here.

use std::path::Path;

use crate::cmd::run_git_unit;
use crate::errors::GitToolingError;

pub async fn push(repo: &Path) -> Result<(), GitToolingError> {
    // `--porcelain` to make stderr deterministic; user-facing errors
    // (rejected, non-fast-forward, no upstream) still raise non-zero.
    run_git_unit(repo, &["push"]).await
}

pub async fn pull(repo: &Path) -> Result<(), GitToolingError> {
    run_git_unit(repo, &["pull", "--ff-only"]).await
}

pub async fn fetch(repo: &Path) -> Result<(), GitToolingError> {
    run_git_unit(repo, &["fetch", "--all", "--prune"]).await
}

/// Pull-then-push. We surface the failing operation's error without trying to
/// be clever — the frontend already has bespoke handling for "push needs sync"
/// scenarios in `GitDiffPanel` and presents a sync button on push failures.
pub async fn sync(repo: &Path) -> Result<(), GitToolingError> {
    pull(repo).await?;
    push(repo).await
}
