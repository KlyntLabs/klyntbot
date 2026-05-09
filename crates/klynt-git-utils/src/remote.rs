//! Remote URL lookup. The frontend uses this to derive the GitHub base URL
//! for "open commit on GitHub" / "open PR" links. We return the URL of the
//! `origin` remote when set, else `None`.

use std::path::Path;

use crate::cmd::run_git;
use crate::errors::GitToolingError;

pub async fn get_origin_url(repo: &Path) -> Result<Option<String>, GitToolingError> {
    match run_git(repo, &["config", "--get", "remote.origin.url"]).await {
        Ok(s) => {
            let trimmed = s.trim();
            Ok(if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            })
        }
        // `git config --get` exits 1 when the key isn't set; treat as `None`.
        Err(_) => Ok(None),
    }
}

/// Add or replace the `origin` remote URL in `repo`.
pub async fn set_origin_url(repo: &Path, url: &str) -> Result<(), GitToolingError> {
    let exists = run_git(repo, &["config", "--get", "remote.origin.url"])
        .await
        .is_ok();
    if exists {
        crate::cmd::run_git_unit(repo, &["remote", "set-url", "origin", url]).await
    } else {
        crate::cmd::run_git_unit(repo, &["remote", "add", "origin", url]).await
    }
}
