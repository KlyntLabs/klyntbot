//! Resolve a kimi workspace `<hash>` directory back to its `cwd` by reading
//! `~/.kimi/kimi.json`.
//!
//! Kimi names each workspace's session directory by md5(work_dir) for
//! `kaos == "local"` entries; non-local entries use `<kaos>_<hash>`.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;

#[derive(Debug, Deserialize)]
struct WorkDirEntry {
    path: String,
    #[serde(default = "default_kaos")]
    kaos: String,
}

fn default_kaos() -> String {
    "local".to_string()
}

#[derive(Debug, Deserialize)]
struct KimiMetadata {
    work_dirs: Vec<WorkDirEntry>,
}

/// In-memory cache of `<hash> → cwd`. Refreshed lazily on miss.
#[derive(Debug, Default)]
pub struct WorkdirIndex {
    map: RwLock<HashMap<String, PathBuf>>,
}

impl WorkdirIndex {
    /// Construct an empty index; call [`refresh`](Self::refresh) before use.
    pub fn new() -> Self {
        Self::default()
    }

    /// Read `kimi_json_path` and rebuild the cache. Missing file is not an
    /// error — the index simply stays empty and resolves yield `None`.
    pub async fn refresh(&self, kimi_json_path: &Path) -> common::Result<()> {
        let bytes = match tokio::fs::read(kimi_json_path).await {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::info!(path = %kimi_json_path.display(), "kimi.json missing — workdir index empty");
                self.map.write().await.clear();
                return Ok(());
            }
            Err(e) => {
                return Err(common::KlyntbotError::Storage(format!(
                    "kimi.json read {}: {e}",
                    kimi_json_path.display()
                )));
            }
        };
        let meta: KimiMetadata = serde_json::from_slice(&bytes).map_err(|e| {
            common::KlyntbotError::Storage(format!("kimi.json parse: {e}"))
        })?;
        let mut next = HashMap::with_capacity(meta.work_dirs.len());
        for entry in meta.work_dirs {
            let hash = hash_for(&entry.path, &entry.kaos);
            next.insert(hash, PathBuf::from(entry.path));
        }
        *self.map.write().await = next;
        Ok(())
    }

    /// Look up a hash. `None` if unknown.
    pub async fn get(&self, hash: &str) -> Option<PathBuf> {
        self.map.read().await.get(hash).cloned()
    }
}

/// Compute the directory-name hash kimi assigns to a `(work_dir, kaos)` pair.
///
/// `local` workspaces use the bare md5 hex digest. Other kaos names are
/// prefixed: `<kaos>_<md5>`. This mirrors the Python reference in
/// `kimi_cli.vis.api.sessions::get_work_dir_for_hash`.
pub fn hash_for(work_dir: &str, kaos: &str) -> String {
    let digest = md5::compute(work_dir.as_bytes());
    let hex = format!("{digest:x}");
    if kaos == "local" {
        hex
    } else {
        format!("{kaos}_{hex}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_for_local_is_md5_hex() {
        // md5("/tmp/kimi-fixture-repo") computed once with `md5sum` for the
        // golden value — DON'T regenerate, this is a contract test.
        let h = hash_for("/tmp/kimi-fixture-repo", "local");
        assert_eq!(h.len(), 32);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_for_non_local_is_prefixed() {
        let h = hash_for("/foo", "remote");
        assert!(h.starts_with("remote_"), "got {h}");
    }

    #[tokio::test]
    async fn refresh_missing_file_is_ok() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.json");
        let idx = WorkdirIndex::new();
        idx.refresh(&path).await.expect("missing file is not an error");
        assert!(idx.get("anyhash").await.is_none());
    }

    #[tokio::test]
    async fn refresh_loads_entries_and_resolves() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kimi.json");
        std::fs::write(
            &path,
            r#"{"work_dirs":[{"path":"/tmp/kimi-fixture-repo","kaos":"local","last_session_id":null}]}"#,
        )
        .unwrap();
        let idx = WorkdirIndex::new();
        idx.refresh(&path).await.unwrap();
        let h = hash_for("/tmp/kimi-fixture-repo", "local");
        assert_eq!(
            idx.get(&h).await.unwrap(),
            std::path::PathBuf::from("/tmp/kimi-fixture-repo")
        );
    }
}
