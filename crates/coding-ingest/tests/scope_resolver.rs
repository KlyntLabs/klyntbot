use coding_ingest::scope_resolver::resolve_scope;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {:?} failed", args);
}

#[test]
fn resolves_canonical_github_id() {
    let dir = TempDir::new().unwrap();
    git(dir.path(), &["init", "-q"]);
    git(
        dir.path(),
        &["remote", "add", "origin", "git@github.com:klynt/bot.git"],
    );
    std::fs::write(dir.path().join("README.md"), "x").unwrap();
    git(dir.path(), &["add", "."]);
    git(
        dir.path(),
        &[
            "-c",
            "user.email=x@x",
            "-c",
            "user.name=x",
            "commit",
            "-qm",
            "x",
        ],
    );
    let scope = resolve_scope(dir.path()).expect("some");
    assert_eq!(scope.repo_id, "github.com/klynt/bot");
    assert_eq!(scope.root, std::fs::canonicalize(dir.path()).unwrap());
    assert!(scope.git_hash.is_some());
}

#[test]
fn falls_back_to_local_for_non_git_paths() {
    let dir = TempDir::new().unwrap();
    let scope = resolve_scope(dir.path()).expect("some");
    assert!(scope.repo_id.starts_with("local:"));
}

#[test]
fn no_remote_uses_local_prefix_with_worktree_basename() {
    let dir = TempDir::new().unwrap();
    git(dir.path(), &["init", "-q"]);
    let scope = resolve_scope(dir.path()).expect("some");
    assert!(scope.repo_id.starts_with("local:"));
}
