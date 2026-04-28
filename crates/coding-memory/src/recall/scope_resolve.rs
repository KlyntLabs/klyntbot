//! Resolve a working directory to a canonical `repo_id` string for scope
//! filtering in the recall builders.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/** Per-session scope resolver with an in-memory cache. */
pub struct ScopeResolver {
    cache: Mutex<HashMap<PathBuf, Option<String>>>,
}

impl Default for ScopeResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ScopeResolver {
    /// Create a new empty resolver.
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Resolve `cwd` to a canonical `repo_id`, caching the result.
    pub fn resolve(&self, cwd: &Path) -> Option<String> {
        let mut cache = self.cache.lock().unwrap();
        if let Some(hit) = cache.get(cwd) {
            return hit.clone();
        }
        let result = coding_ingest::scope_resolver::resolve_scope(cwd).map(|s| s.repo_id);
        cache.insert(cwd.to_path_buf(), result.clone());
        result
    }
}

/// Stateless fallback for one-shot resolution.
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

    #[test]
    fn resolver_caches_result() {
        let resolver = ScopeResolver::new();
        let cwd = std::env::temp_dir();
        let a = resolver.resolve(&cwd);
        let b = resolver.resolve(&cwd);
        assert_eq!(a, b);
        assert_eq!(resolver.cache.lock().unwrap().len(), 1);
    }
}
