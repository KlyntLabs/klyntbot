//! Git ghost-commit snapshots for code-session rewind.
//! Ported from codex `git-utils/src/ghost_commits.rs`.

mod errors;
pub use errors::GitToolingError;

mod ghost_commits;
pub use ghost_commits::{create_ghost_commit, restore_ghost_commit, GhostSnapshotConfig};

mod repo;
pub use repo::{get_git_repo_root, is_inside_git_repo};

use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

type CommitID = String;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GhostCommit {
    id: CommitID,
    parent: Option<CommitID>,
    preexisting_untracked_files: Vec<PathBuf>,
    preexisting_untracked_dirs: Vec<PathBuf>,
}

impl GhostCommit {
    pub fn new(
        id: CommitID,
        parent: Option<CommitID>,
        preexisting_untracked_files: Vec<PathBuf>,
        preexisting_untracked_dirs: Vec<PathBuf>,
    ) -> Self {
        Self {
            id,
            parent,
            preexisting_untracked_files,
            preexisting_untracked_dirs,
        }
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn parent(&self) -> Option<&str> {
        self.parent.as_deref()
    }
    pub fn preexisting_untracked_files(&self) -> &[PathBuf] {
        &self.preexisting_untracked_files
    }
    pub fn preexisting_untracked_dirs(&self) -> &[PathBuf] {
        &self.preexisting_untracked_dirs
    }
}

impl fmt::Display for GhostCommit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

#[cfg(test)]
mod ghost_commit_struct_tests {
    use super::*;

    #[test]
    fn ghost_commit_round_trips_serde() {
        let g = GhostCommit::new(
            "abc123".into(),
            Some("def456".into()),
            vec![PathBuf::from("untracked.txt")],
            vec![PathBuf::from("untracked_dir")],
        );
        let s = serde_json::to_string(&g).unwrap();
        let back: GhostCommit = serde_json::from_str(&s).unwrap();
        assert_eq!(g, back);
        assert_eq!(g.id(), "abc123");
        assert_eq!(g.parent(), Some("def456"));
    }
}

#[cfg(test)]
mod repo_detect_tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn returns_false_outside_repo() {
        let tmp = TempDir::new().unwrap();
        assert!(!is_inside_git_repo(tmp.path()).await.unwrap());
    }

    #[tokio::test]
    async fn returns_true_inside_repo() {
        let tmp = TempDir::new().unwrap();
        tokio::process::Command::new("git")
            .arg("init")
            .current_dir(tmp.path())
            .output()
            .await
            .unwrap();
        assert!(is_inside_git_repo(tmp.path()).await.unwrap());
    }

    #[tokio::test]
    async fn returns_repo_root_for_subdir() {
        let tmp = TempDir::new().unwrap();
        tokio::process::Command::new("git")
            .arg("init")
            .current_dir(tmp.path())
            .output()
            .await
            .unwrap();
        let sub = tmp.path().join("a/b/c");
        std::fs::create_dir_all(&sub).unwrap();
        let root = get_git_repo_root(&sub).await.unwrap();
        // git resolves symlinks (e.g. /var → /private/var on macOS)
        let canonical = std::fs::canonicalize(tmp.path()).unwrap();
        assert_eq!(root, canonical);
    }
}
