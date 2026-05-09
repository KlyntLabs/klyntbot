//! Branch listing + checkout/create. Uses `git for-each-ref` with a custom
//! format so we can sort by `committerdate` server-side and avoid a round-trip
//! per branch.

use std::path::Path;

use crate::cmd::{run_git, run_git_unit};
use crate::errors::GitToolingError;
use crate::types::BranchInfo;

/// All local branches, sorted by latest committer date (newest first). The
/// frontend's branch picker shows recently-active branches at the top.
pub async fn list(repo: &Path) -> Result<Vec<BranchInfo>, GitToolingError> {
    // Format: `<short-name>\x1f<committerdate-unix>`
    let format = "%(refname:short)\x1f%(committerdate:unix)";
    let raw = run_git(
        repo,
        &[
            "for-each-ref",
            "--sort=-committerdate",
            &format!("--format={format}"),
            "refs/heads",
        ],
    )
    .await?;
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut cols = line.splitn(2, '\x1f');
        let name = cols.next().unwrap_or("").to_string();
        let ts = cols.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
        if name.is_empty() {
            continue;
        }
        out.push(BranchInfo {
            name,
            last_commit: ts,
        });
    }
    Ok(out)
}

pub async fn checkout(repo: &Path, name: &str) -> Result<(), GitToolingError> {
    run_git_unit(repo, &["checkout", name]).await
}

pub async fn create(repo: &Path, name: &str) -> Result<(), GitToolingError> {
    run_git_unit(repo, &["checkout", "-b", name]).await
}
