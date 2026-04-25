//! Session-summary repo — caches the 200-token markdown surface emitted by
//! the session-end light pass. Read by the Phase-4 SessionStart renderer's
//! "Open threads" section.

use common::{KlyntbotError, Result};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummaryRow {
    /// Id.
    pub id: String,
    /// Session id.
    pub session_id: String,
    /// Repo id.
    pub repo_id: Option<String>,
    /// When summarised.
    pub summarised_at: Timestamp,
    /// Markdown body.
    pub summary_md: String,
    /// Estimated tokens.
    pub token_count: u32,
}

/// Repo.
#[derive(Debug, Clone)]
pub struct SessionSummaryRepo {
    pool: storage::StoragePool,
}

impl SessionSummaryRepo {
    /// Construct.
    pub fn new(pool: storage::StoragePool) -> Self {
        Self { pool }
    }

    /// Insert one row.
    pub async fn insert(&self, row: &SessionSummaryRow) -> Result<()> {
        sqlx::query(
            "INSERT INTO session_summaries \
             (id, session_id, repo_id, summarised_at, summary_md, token_count, actor_id) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'local_user')",
        )
        .bind(&row.id)
        .bind(&row.session_id)
        .bind(&row.repo_id)
        .bind(row.summarised_at.to_string())
        .bind(&row.summary_md)
        .bind(row.token_count as i64)
        .execute(self.pool.inner())
        .await
        .map_err(|e| KlyntbotError::Storage(format!("session_summaries insert: {e}")))?;
        Ok(())
    }

    /// Get the most-recent summary for a session.
    pub async fn latest_for_session(&self, session_id: &str) -> Result<Option<SessionSummaryRow>> {
        let row: Option<(String, String, Option<String>, String, String, i64)> = sqlx::query_as(
            "SELECT id, session_id, repo_id, summarised_at, summary_md, token_count \
             FROM session_summaries WHERE session_id = ?1 \
             ORDER BY summarised_at DESC LIMIT 1",
        )
        .bind(session_id)
        .fetch_optional(self.pool.inner())
        .await
        .map_err(|e| KlyntbotError::Storage(format!("session_summaries read: {e}")))?;
        Ok(row.map(|(id, session_id, repo_id, summarised_at, summary_md, token_count)| {
            SessionSummaryRow {
                id,
                session_id,
                repo_id,
                summarised_at: summarised_at.parse().unwrap_or_else(|_| Timestamp::now()),
                summary_md,
                token_count: token_count as u32,
            }
        }))
    }

    /// List recent summaries for a repo.
    pub async fn recent_for_repo(
        &self,
        repo_id: &str,
        limit: u32,
    ) -> Result<Vec<SessionSummaryRow>> {
        let rows: Vec<(String, String, Option<String>, String, String, i64)> = sqlx::query_as(
            "SELECT id, session_id, repo_id, summarised_at, summary_md, token_count \
             FROM session_summaries WHERE repo_id = ?1 \
             ORDER BY summarised_at DESC LIMIT ?2",
        )
        .bind(repo_id)
        .bind(limit as i64)
        .fetch_all(self.pool.inner())
        .await
        .map_err(|e| KlyntbotError::Storage(format!("session_summaries list: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|(id, session_id, repo_id, summarised_at, summary_md, token_count)| {
                SessionSummaryRow {
                    id,
                    session_id,
                    repo_id,
                    summarised_at: summarised_at
                        .parse()
                        .unwrap_or_else(|_| Timestamp::now()),
                    summary_md,
                    token_count: token_count as u32,
                }
            })
            .collect())
    }

    /// Build a fresh row id.
    #[must_use]
    pub fn new_row_id() -> String {
        format!("sumsess_{}", Uuid::new_v4().simple())
    }

    /// Pool ref.
    pub fn pool(&self) -> &storage::StoragePool {
        &self.pool
    }
}
