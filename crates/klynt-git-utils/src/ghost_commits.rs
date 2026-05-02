use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tempfile::TempDir;
use tokio::process::Command;

use crate::errors::GitToolingError;
use crate::repo::get_git_repo_root;
use crate::GhostCommit;

/// Configuration for ghost-snapshot creation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostSnapshotConfig {
    /// Skip files larger than this many bytes.
    pub max_file_bytes: u64,
    /// Skip directories with more than this many entries.
    pub max_dir_entries: usize,
    /// Path components to always exclude.
    pub excluded_path_components: Vec<String>,
}

impl Default for GhostSnapshotConfig {
    fn default() -> Self {
        Self {
            max_file_bytes: 10 * 1024 * 1024, // 10 MiB
            max_dir_entries: 200,
            excluded_path_components: vec![
                "node_modules".into(),
                ".venv".into(),
                "venv".into(),
                "target".into(),
                "dist".into(),
                "build".into(),
                ".next".into(),
                ".cache".into(),
            ],
        }
    }
}

/// Create a ghost commit capturing the current state of the working tree at `repo_path`.
/// Returns the new GhostCommit. Does NOT touch any branch refs.
pub async fn create_ghost_commit(
    repo_path: &Path,
    config: &GhostSnapshotConfig,
) -> Result<GhostCommit, GitToolingError> {
    let root = get_git_repo_root(repo_path).await?;

    // 1. Get the parent SHA (HEAD), if any.
    let parent = git_rev_parse_head(&root).await?;

    // 2. Create a temp index so we don't touch the user's staging area.
    let tmp_index_dir = TempDir::new()?;
    let tmp_index_path = tmp_index_dir.path().join("index");
    let index_env = ("GIT_INDEX_FILE", tmp_index_path.as_path());

    // 3. If we have a parent, populate the temp index from HEAD.
    if let Some(parent_sha) = &parent {
        run_git(&root, &["read-tree", parent_sha], Some(&[index_env])).await?;
    }

    // 4. Snapshot which files are currently untracked (we'll need this for restore).
    let preexisting_untracked = list_untracked_files(&root).await?;

    // 5. Add everything respecting size/dir-count/exclude filters.
    let to_add = collect_files_to_snapshot(&root, config).await?;
    if !to_add.is_empty() {
        // Use --add to stage; pass paths as args.
        let mut args: Vec<String> = vec!["add".into(), "--force".into(), "--".into()];
        args.extend(to_add.iter().map(|p| p.to_string_lossy().into_owned()));
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        run_git(&root, &arg_refs, Some(&[index_env])).await?;
    }

    // 6. Write the tree.
    let tree_sha_out = run_git(&root, &["write-tree"], Some(&[index_env])).await?;
    let tree_sha = decode_utf8_output(tree_sha_out, "git write-tree")?;

    // 7. Commit-tree with the message "klynt-snapshot".
    let mut commit_args: Vec<String> = vec!["commit-tree".into(), tree_sha.clone()];
    if let Some(p) = &parent {
        commit_args.push("-p".into());
        commit_args.push(p.clone());
    }
    commit_args.push("-m".into());
    commit_args.push("klynt-snapshot".into());
    let arg_refs: Vec<&str> = commit_args.iter().map(|s| s.as_str()).collect();
    let commit_sha_out = run_git(&root, &arg_refs, Some(&[index_env])).await?;
    let commit_sha = decode_utf8_output(commit_sha_out, "git commit-tree")?;

    Ok(GhostCommit::new(
        commit_sha,
        parent,
        preexisting_untracked,
        Vec::new(),
    ))
}

/// Restore the working tree to the state captured by `ghost`.
/// - Files in the ghost tree are restored to their snapshotted content.
/// - Files that did NOT exist when the ghost was captured are deleted
///   (anything new since the snapshot).
/// - Pre-existing untracked files (recorded in the ghost) are kept.
pub async fn restore_ghost_commit(
    repo_path: &Path,
    ghost: &GhostCommit,
) -> Result<(), GitToolingError> {
    let root = get_git_repo_root(repo_path).await?;

    // Step A: hard-restore the worktree (but NOT the index) to the ghost tree.
    run_git(
        &root,
        &["restore", "--source", ghost.id(), "--worktree", "--", "."],
        None,
    )
    .await?;

    // Step B: delete files that exist now but weren't in the ghost tree
    //         AND weren't preexisting untracked files we should keep.
    let in_ghost_tree = list_paths_in_tree(&root, ghost.id()).await?;
    let in_ghost_set: HashSet<PathBuf> = in_ghost_tree.into_iter().collect();
    let preexisting: HashSet<&PathBuf> = ghost.preexisting_untracked_files().iter().collect();

    for entry in walkdir::WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git")
    {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry.path().strip_prefix(&root)?.to_path_buf();
        if in_ghost_set.contains(&rel) {
            continue;
        }
        if preexisting.contains(&rel) {
            continue;
        }
        // This is a file that appeared after the snapshot; remove it.
        let _ = std::fs::remove_file(entry.path());
    }
    Ok(())
}

async fn list_paths_in_tree(root: &Path, sha: &str) -> Result<Vec<PathBuf>, GitToolingError> {
    let out = run_git(root, &["ls-tree", "-r", "--name-only", "-z", sha], None).await?;
    Ok(out
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| PathBuf::from(String::from_utf8_lossy(s).into_owned()))
        .collect())
}

async fn git_rev_parse_head(root: &Path) -> Result<Option<String>, GitToolingError> {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .await?;
    if !out.status.success() {
        return Ok(None); // No HEAD yet (empty repo).
    }
    Ok(Some(String::from_utf8_lossy(&out.stdout).trim().to_string()))
}

async fn list_untracked_files(root: &Path) -> Result<Vec<PathBuf>, GitToolingError> {
    let out = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard", "-z"])
        .current_dir(root)
        .output()
        .await?;
    if !out.status.success() {
        return Err(GitToolingError::GitCommand {
            command: "git ls-files".into(),
            status: out.status,
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    Ok(out
        .stdout
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| PathBuf::from(String::from_utf8_lossy(s).into_owned()))
        .collect())
}

async fn collect_files_to_snapshot(
    root: &Path,
    config: &GhostSnapshotConfig,
) -> Result<Vec<PathBuf>, GitToolingError> {
    let exclude_set: HashSet<&str> = config
        .excluded_path_components
        .iter()
        .map(|s| s.as_str())
        .collect();

    let mut out = Vec::new();
    let mut dir_counts: std::collections::HashMap<PathBuf, usize> = std::collections::HashMap::new();
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            !e.file_name()
                .to_str()
                .map(|n| n == ".git" || exclude_set.contains(n))
                .unwrap_or(false)
        })
    {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let parent = entry.path().parent().unwrap_or(root);
        let count = *dir_counts.entry(parent.to_path_buf()).or_insert_with(|| {
            std::fs::read_dir(parent).map(|rd| rd.count()).unwrap_or(0)
        });
        if count > config.max_dir_entries {
            continue;
        }
        let meta = entry.metadata()?;
        if meta.len() > config.max_file_bytes {
            continue;
        }
        let rel = entry.path().strip_prefix(root)?.to_path_buf();
        out.push(rel);
    }
    Ok(out)
}

async fn run_git(
    cwd: &Path,
    args: &[&str],
    extra_env: Option<&[(&str, &Path)]>,
) -> Result<Vec<u8>, GitToolingError> {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(cwd);
    if let Some(envs) = extra_env {
        for (k, v) in envs {
            cmd.env(k, v);
        }
    }
    let out = cmd.output().await?;
    if !out.status.success() {
        return Err(GitToolingError::GitCommand {
            command: format!("git {}", args.join(" ")),
            status: out.status,
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    Ok(out.stdout)
}

fn decode_utf8_output(out: Vec<u8>, command: &str) -> Result<String, GitToolingError> {
    String::from_utf8(out)
        .map_err(|e| GitToolingError::GitOutputUtf8 {
            command: command.into(),
            source: e,
        })
        .map(|s| s.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn init_repo(dir: &Path) {
        for args in [
            &["init"][..],
            &["config", "user.email", "test@klynt.local"][..],
            &["config", "user.name", "Test"][..],
        ] {
            run_git(dir, args, None).await.unwrap();
        }
    }

    async fn commit_file(dir: &Path, name: &str, body: &str) -> String {
        std::fs::write(dir.join(name), body).unwrap();
        run_git(dir, &["add", name], None).await.unwrap();
        run_git(dir, &["commit", "-m", "msg"], None)
            .await
            .unwrap();
        String::from_utf8(run_git(dir, &["rev-parse", "HEAD"], None).await.unwrap())
            .unwrap()
            .trim()
            .to_string()
    }

    #[tokio::test]
    async fn create_in_empty_repo_has_no_parent() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path()).await;
        std::fs::write(tmp.path().join("a.txt"), "hello").unwrap();
        let ghost = create_ghost_commit(tmp.path(), &GhostSnapshotConfig::default())
            .await
            .unwrap();
        assert!(ghost.parent().is_none());
        assert!(!ghost.id().is_empty());
    }

    #[tokio::test]
    async fn create_with_existing_head_records_parent() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path()).await;
        let head = commit_file(tmp.path(), "a.txt", "v1").await;
        std::fs::write(tmp.path().join("a.txt"), "v2-uncommitted").unwrap();
        let ghost = create_ghost_commit(tmp.path(), &GhostSnapshotConfig::default())
            .await
            .unwrap();
        assert_eq!(ghost.parent(), Some(head.as_str()));
    }

    #[tokio::test]
    async fn excluded_dirs_are_skipped() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path()).await;
        std::fs::create_dir(tmp.path().join("node_modules")).unwrap();
        std::fs::write(tmp.path().join("node_modules/big.js"), "x".repeat(1_000_000)).unwrap();
        std::fs::write(tmp.path().join("a.txt"), "small").unwrap();
        create_ghost_commit(tmp.path(), &GhostSnapshotConfig::default())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn large_files_are_skipped() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path()).await;
        std::fs::write(tmp.path().join("huge.bin"), vec![0u8; 11 * 1024 * 1024]).unwrap();
        std::fs::write(tmp.path().join("ok.txt"), "small").unwrap();
        let cfg = GhostSnapshotConfig::default();
        let ghost = create_ghost_commit(tmp.path(), &cfg).await.unwrap();
        let out = run_git(tmp.path(), &["ls-tree", "-r", ghost.id()], None)
            .await
            .unwrap();
        let listing = String::from_utf8_lossy(&out);
        assert!(!listing.contains("huge.bin"), "huge file leaked: {listing}");
        assert!(listing.contains("ok.txt"));
    }

    #[tokio::test]
    async fn restore_round_trip() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path()).await;
        commit_file(tmp.path(), "a.txt", "v1").await;
        std::fs::write(tmp.path().join("a.txt"), "v2-uncommitted").unwrap();
        let ghost = create_ghost_commit(tmp.path(), &GhostSnapshotConfig::default())
            .await
            .unwrap();

        // Mutate further.
        std::fs::write(tmp.path().join("a.txt"), "v3-mutated-after-snapshot").unwrap();
        std::fs::write(tmp.path().join("new.txt"), "added after snapshot").unwrap();

        restore_ghost_commit(tmp.path(), &ghost).await.unwrap();

        let restored = std::fs::read_to_string(tmp.path().join("a.txt")).unwrap();
        assert_eq!(restored, "v2-uncommitted");
        // new.txt was created AFTER the snapshot — should be removed.
        assert!(
            !tmp.path().join("new.txt").exists(),
            "post-snapshot file should be removed"
        );
    }

    #[tokio::test]
    async fn restore_keeps_files_that_predated_the_snapshot() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path()).await;
        commit_file(tmp.path(), "a.txt", "v1").await;
        // An untracked file existed BEFORE we snapshotted.
        std::fs::write(tmp.path().join("preexisting.log"), "existed").unwrap();
        let ghost = create_ghost_commit(tmp.path(), &GhostSnapshotConfig::default())
            .await
            .unwrap();
        // Mutate after.
        std::fs::write(tmp.path().join("preexisting.log"), "modified after").unwrap();
        restore_ghost_commit(tmp.path(), &ghost).await.unwrap();
        // File still exists (it was in the snapshot).
        assert!(tmp.path().join("preexisting.log").exists());
    }
}
