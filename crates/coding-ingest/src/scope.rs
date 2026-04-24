//! `RepoScope` — canonical repo identity for an `AgentEvent`.
//!
//! Derived from `cwd` via `git rev-parse` + remote origin URL; cached per
//! session. `repo_id` is a sanitized slug, `root` is an absolute path.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Canonical repo identity attached to every `AgentEvent` when detectable.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepoScope {
    /// Canonical repo id — e.g. `github.com/klynt/bot` or `local:sanitized-path`.
    pub repo_id: String,
    /// Absolute path to the working tree root.
    pub root: PathBuf,
    /// Current HEAD commit hash if available at event time.
    pub git_hash: Option<String>,
    /// Current branch if available at event time.
    pub branch: Option<String>,
}
