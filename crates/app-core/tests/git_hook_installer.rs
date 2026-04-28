//! Phase 6 — git post-commit hook installer round-trip.

use app_core::coding_memory::git_hook_installer;

#[test]
fn installs_and_uninstalls() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(repo.join(".git").join("hooks")).unwrap();

    git_hook_installer::install(&repo).expect("install");
    let hook = repo.join(".git").join("hooks").join("post-commit");
    assert!(hook.exists(), "post-commit hook should be created");

    let body = std::fs::read_to_string(&hook).unwrap();
    assert!(
        body.contains("klyntbot-hook git-post-commit"),
        "hook should invoke klyntbot-hook git-post-commit"
    );

    git_hook_installer::uninstall(&repo).expect("uninstall");
    assert!(!hook.exists(), "uninstall should remove the hook");
}
