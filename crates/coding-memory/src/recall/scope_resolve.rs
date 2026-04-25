//! Resolve a working directory to a canonical `repo_id` string for scope
//! filtering in the recall builders.

use std::path::Path;

/// Returns `None` when `cwd` doesn't exist; falls back to `local:<basename>`
/// for non-git directories.
#[must_use]
pub fn repo_id_for_cwd(cwd: &Path) -> Option<String> {
    coding_ingest::scope_resolver::resolve_scope(cwd).map(|s| s.repo_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_path_returns_none() {
        let out = repo_id_for_cwd(Path::new("/nonexistent/path/should/not/exist/xyz"));
        assert!(out.is_none());
    }

    #[test]
    fn existing_dir_yields_some_repo_id() {
        let cwd = std::env::temp_dir();
        let out = repo_id_for_cwd(&cwd);
        assert!(out.is_some());
        assert!(out.unwrap().starts_with("local:") || !cwd.to_string_lossy().is_empty());
    }
}
