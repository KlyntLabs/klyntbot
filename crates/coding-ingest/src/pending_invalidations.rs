//! Queue of `GitCommit` events that arrived while desktop was offline.

use crate::event::{AgentEvent, EventKind};
use storage::StoragePool;
use uuid::Uuid;

/// Repository.
#[derive(Debug, Clone)]
pub struct PendingInvalidationsRepo {
    pool: StoragePool,
}

impl PendingInvalidationsRepo {
    /// Construct.
    #[must_use]
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }

    /// Append a single GitCommit event row.
    pub async fn append(&self, event: &AgentEvent) -> common::Result<()> {
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
        let id = Uuid::new_v4().to_string();
        let files = serde_json::to_string(changed_files)
            .map_err(|e| common::KlyntbotError::Storage(format!("files json: {e}")))?;
        sqlx::query(
            "INSERT INTO pending_invalidations \
             (id, repo_root, commit_hash, parent_hash, changed_files) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(id)
        .bind(repo_root.to_string_lossy().to_string())
        .bind(commit_hash)
        .bind(parent_hash)
        .bind(files)
        .execute(self.pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("pending append: {e}")))?;
        Ok(())
    }

    /// All unprocessed rows as `(row_id, AgentEvent)`.
    pub async fn drain_unprocessed(&self) -> common::Result<Vec<(String, AgentEvent)>> {
        let rows: Vec<(String, String, String, Option<String>, String)> = sqlx::query_as(
            "SELECT id, repo_root, commit_hash, parent_hash, changed_files \
             FROM pending_invalidations \
             WHERE processed_at IS NULL \
             ORDER BY received_at ASC",
        )
        .fetch_all(self.pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("pending drain: {e}")))?;

        let mut out = Vec::new();
        for (id, repo_root, commit_hash, parent_hash, files_json) in rows {
            let changed_files: Vec<std::path::PathBuf> =
                serde_json::from_str(&files_json).unwrap_or_default();
            let event = AgentEvent::V1(crate::event::AgentEventV1 {
                id: Uuid::new_v4(),
                source: crate::event::AgentSource::ClaudeCode,
                session_id: format!("git:{commit_hash}"),
                turn_id: None,
                cwd: std::path::PathBuf::from(&repo_root),
                repo: None,
                occurred_at: jiff::Timestamp::now(),
                kind: EventKind::GitCommit {
                    commit_hash,
                    parent_hash,
                    repo_root: std::path::PathBuf::from(&repo_root),
                    changed_files,
                },
            });
            out.push((id, event));
        }
        Ok(out)
    }

    /// Mark a row processed.
    pub async fn mark_processed(&self, id: &str) -> common::Result<()> {
        sqlx::query(
            "UPDATE pending_invalidations SET processed_at = datetime('now') WHERE id = ?1",
        )
        .bind(id)
        .execute(self.pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("mark processed: {e}")))?;
        Ok(())
    }
}
