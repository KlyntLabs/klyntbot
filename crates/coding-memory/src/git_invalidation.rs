//! `GitInvalidationHandlerImpl` — diffs parent commit, runs tree-sitter on changed
//! files, invalidates facts whose anchored symbols vanished.
//!
//! **Note on location:** This impl lives in `coding-memory` (rather than
//! `coding-ingest`) because it needs the `SymbolExtractor` trait which is defined
//! here. `coding-ingest` re-exports the trait so the daemon can dispatch through
//! it without a circular dependency.

use coding_ingest::event::{AgentEvent, EventKind};
use coding_ingest::git_invalidation::GitInvalidationHandler;
use std::sync::Arc;
use storage::StoragePool;

/// Per-file symbol snapshot.
struct FileDiff {
    rel: std::path::PathBuf,
    old_symbols: Vec<crate::scope::AnchoredSymbol>,
    new_symbols: Vec<crate::scope::AnchoredSymbol>,
}

/// Default impl backed by `git2` + `coding_memory::SymbolExtractor`.
#[derive(Debug, Clone)]
pub struct GitInvalidationHandlerImpl {
    pool: StoragePool,
    extractor: Arc<dyn crate::symbols::SymbolExtractor>,
}

impl GitInvalidationHandlerImpl {
    /// Construct.
    #[must_use]
    pub fn new(pool: StoragePool, extractor: Arc<dyn crate::symbols::SymbolExtractor>) -> Self {
        Self { pool, extractor }
    }

    /// Build per-file diffs synchronously; git2 types are dropped before returning.
    fn build_diffs(
        &self,
        repo_root: &std::path::Path,
        changed_files: &[std::path::PathBuf],
        commit_hash: &str,
        parent_hash: &Option<String>,
    ) -> common::Result<Vec<FileDiff>> {
        let repo = git2::Repository::open(repo_root)
            .map_err(|e| common::KlyntbotError::Storage(format!("git open: {e}")))?;
        let parent_tree = if let Some(ref p) = parent_hash {
            let oid = git2::Oid::from_str(p)
                .map_err(|e| common::KlyntbotError::Storage(format!("parent oid: {e}")))?;
            Some(
                repo.find_commit(oid)
                    .and_then(|c| c.tree())
                    .map_err(|e| common::KlyntbotError::Storage(format!("parent tree: {e}")))?,
            )
        } else {
            None
        };

        let mut diffs = Vec::new();
        for rel in changed_files {
            let abs = repo_root.join(rel);
            let new_source = std::fs::read_to_string(&abs).unwrap_or_default();
            let old_source: String = parent_tree
                .as_ref()
                .and_then(|t| t.get_path(rel).ok())
                .and_then(|entry| repo.find_blob(entry.id()).ok())
                .map(|blob| String::from_utf8_lossy(blob.content()).to_string())
                .unwrap_or_default();
            let new_symbols = self.extractor.extract(&abs, &new_source, commit_hash);
            let old_symbols = if let Some(ref parent) = parent_hash {
                self.extractor.extract(&abs, &old_source, parent)
            } else {
                Vec::new()
            };
            diffs.push(FileDiff {
                rel: rel.clone(),
                old_symbols,
                new_symbols,
            });
        }
        Ok(diffs)
    }
}

#[async_trait::async_trait]
impl GitInvalidationHandler for GitInvalidationHandlerImpl {
    async fn handle(&self, event: &AgentEvent) -> common::Result<()> {
        let AgentEvent::V1(v1) = event;
        let EventKind::GitCommit {
            commit_hash,
            parent_hash,
            repo_root,
            changed_files,
        } = &v1.kind
        else {
            return Ok(());
        };

        let diffs = self.build_diffs(repo_root, changed_files, commit_hash, parent_hash)?;

        let mut affected: Vec<(String, String)> = Vec::new();

        for diff in &diffs {
            let pattern = format!("%{}%", diff.rel.to_string_lossy());
            let rows: Vec<(String, String)> = sqlx::query_as(
                "SELECT id, metadata FROM semantic_facts \
                 WHERE metadata LIKE ?1 \
                 UNION ALL \
                 SELECT id, metadata FROM episodic_memories \
                 WHERE metadata LIKE ?1",
            )
            .bind(&pattern)
            .fetch_all(self.pool.inner())
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("scan: {e}")))?;

            for (id, metadata) in rows {
                let parsed: serde_json::Value = match metadata.parse() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let anchors: Vec<crate::scope::AnchoredSymbol> =
                    match serde_json::from_value(parsed["anchoredSymbols"].clone()) {
                        Ok(a) => a,
                        Err(_) => continue,
                    };
                let mut deleted = false;
                let mut modified = false;
                for anchor in anchors.iter().filter(|a| a.file_path == diff.rel) {
                    let in_old = diff
                        .old_symbols
                        .iter()
                        .any(|s| s.symbol == anchor.symbol && s.kind == anchor.kind);
                    let in_new = diff
                        .new_symbols
                        .iter()
                        .any(|s| s.symbol == anchor.symbol && s.kind == anchor.kind);
                    if in_old && !in_new {
                        deleted = true;
                    } else if in_old && in_new {
                        modified = true;
                    }
                }
                if deleted {
                    affected.push((id, "delete".into()));
                } else if modified {
                    affected.push((id, "stale".into()));
                }
            }
        }

        for (id, mode) in affected {
            match mode.as_str() {
                "delete" => {
                    sqlx::query(
                        "UPDATE semantic_facts SET valid_until = COALESCE(valid_until, datetime('now')) WHERE id = ?1",
                    )
                    .bind(&id)
                    .execute(self.pool.inner())
                    .await
                    .ok();
                    sqlx::query(
                        "UPDATE episodic_memories SET valid_until = COALESCE(valid_until, datetime('now')) WHERE id = ?1",
                    )
                    .bind(&id)
                    .execute(self.pool.inner())
                    .await
                    .ok();
                }
                "stale" => {
                    let _ = update_metadata_status(&self.pool, &id, "stale_candidate").await;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

async fn update_metadata_status(pool: &StoragePool, id: &str, status: &str) -> common::Result<()> {
    for table in ["semantic_facts", "episodic_memories"] {
        let row: Option<(String,)> =
            sqlx::query_as(&format!("SELECT metadata FROM {table} WHERE id = ?1"))
                .bind(id)
                .fetch_optional(pool.inner())
                .await
                .map_err(|e| common::KlyntbotError::Storage(format!("read {table}: {e}")))?;
        if let Some((meta,)) = row {
            let mut parsed: serde_json::Value = meta.parse().unwrap_or(serde_json::json!({}));
            if let Some(obj) = parsed.as_object_mut() {
                obj.insert(
                    "status".into(),
                    serde_json::Value::String(status.to_string()),
                );
            }
            let _ = sqlx::query(&format!("UPDATE {table} SET metadata = ?1 WHERE id = ?2"))
                .bind(parsed.to_string())
                .bind(id)
                .execute(pool.inner())
                .await;
        }
    }
    Ok(())
}
