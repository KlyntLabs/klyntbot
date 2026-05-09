//! Shared shell-out helper. Every git op in this crate runs through `run_git`
//! so error reporting, command logging, and exit-code handling are uniform.

use std::path::Path;
use tokio::process::Command;

use crate::errors::GitToolingError;

/// Run `git <args>` in `cwd`. Returns stdout (UTF-8) on success; on failure,
/// returns a `GitCommand` error carrying status + stderr verbatim.
pub async fn run_git(cwd: &Path, args: &[&str]) -> Result<String, GitToolingError> {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .await?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        return Err(GitToolingError::GitCommand {
            command: format!("git {}", args.join(" ")),
            status: out.status,
            stderr,
        });
    }
    String::from_utf8(out.stdout).map_err(|e| GitToolingError::GitOutputUtf8 {
        command: format!("git {}", args.join(" ")),
        source: e,
    })
}

/// Run `git <args>` and return raw stdout bytes — used by binary-aware ops
/// like `git show <sha>:<path>` where the payload may not be UTF-8.
pub async fn run_git_bytes(cwd: &Path, args: &[&str]) -> Result<Vec<u8>, GitToolingError> {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .await?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        return Err(GitToolingError::GitCommand {
            command: format!("git {}", args.join(" ")),
            status: out.status,
            stderr,
        });
    }
    Ok(out.stdout)
}

/// Run `git <args>` for an op whose only signal is success vs. failure
/// (commit, push, pull, checkout, etc.). Discards stdout.
pub async fn run_git_unit(cwd: &Path, args: &[&str]) -> Result<(), GitToolingError> {
    run_git(cwd, args).await.map(|_| ())
}
