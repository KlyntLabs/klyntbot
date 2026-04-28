//! Git hook installer — writes `.git/hooks/post-commit` per repo.

use std::io::Write;
use std::path::Path;

const POST_COMMIT_HOOK: &str = r##"#!/bin/sh
# klyntbot git post-commit hook
# Auto-installed by coding-memory subsystem.

echo '{"commit_hash":"'$(git rev-parse HEAD)'","parent_hash":"'$(git rev-parse HEAD^ 2>/dev/null || echo null)'","repo_root":"'$(pwd)'","changed_files":['$(git diff-tree --no-commit-id --name-only -r HEAD | sed 's/^/"/;s/$/",/' | sed '$ s/,$//')']}' | klyntbot-hook git-post-commit
"##;

/// Install the post-commit hook for the given repo root.
pub fn install(repo_root: &Path) -> std::io::Result<()> {
    let hooks_dir = repo_root.join(".git").join("hooks");
    std::fs::create_dir_all(&hooks_dir)?;
    let path = hooks_dir.join("post-commit");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)?;
    file.write_all(POST_COMMIT_HOOK.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

/// Uninstall the post-commit hook for the given repo root.
pub fn uninstall(repo_root: &Path) -> std::io::Result<()> {
    let path = repo_root.join(".git").join("hooks").join("post-commit");
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_and_uninstalls() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo.join(".git").join("hooks")).unwrap();
        install(&repo).unwrap();
        let hook = repo.join(".git").join("hooks").join("post-commit");
        assert!(hook.exists());
        let content = std::fs::read_to_string(&hook).unwrap();
        assert!(content.contains("klyntbot-hook git-post-commit"));
        uninstall(&repo).unwrap();
        assert!(!hook.exists());
    }
}
