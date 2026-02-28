//! Session repository — sessions + session_messages tables.

use chrono::Utc;
use sqlx::SqlitePool;

use crate::error::{OptionExt, StorageError};
use crate::rows::session::{SessionListRow, SessionMessageRow, SessionRow};

/// Repository for session and message persistence.
#[derive(Debug, Clone)]
pub struct SessionRepo {
    pool: SqlitePool,
}

impl SessionRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Upsert a session — inserts on first call, updates `updated_at` on conflict.
    pub async fn upsert_session(
        &self,
        key: &str,
        metadata: &serde_json::Value,
    ) -> Result<SessionRow, StorageError> {
        let now = Utc::now();
        let row = sqlx::query_as::<_, SessionRow>(
            "INSERT INTO sessions (key, metadata, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (key) DO UPDATE SET updated_at = ?4
             RETURNING *",
        )
        .bind(key)
        .bind(metadata)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Get a session by key.
    pub async fn get_session(&self, key: &str) -> Result<SessionRow, StorageError> {
        sqlx::query_as::<_, SessionRow>("SELECT * FROM sessions WHERE key = ?1")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_not_found(&format!("session '{}'", key))
    }

    /// List all sessions with message counts, ordered by updated_at descending.
    pub async fn list_sessions(&self) -> Result<Vec<SessionListRow>, StorageError> {
        let rows = sqlx::query_as::<_, SessionListRow>(
            "SELECT s.key, s.metadata, s.created_at, s.updated_at,
                    COALESCE(counts.cnt, 0) AS message_count
             FROM sessions s
             LEFT JOIN (
                 SELECT session_key, COUNT(*) AS cnt
                 FROM session_messages
                 GROUP BY session_key
             ) counts ON counts.session_key = s.key
             ORDER BY s.updated_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Count total sessions.
    pub async fn count_sessions(&self) -> Result<i64, StorageError> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    /// Add a message to a session.
    ///
    /// Uses a CTE to touch `sessions.updated_at` and insert the message
    /// in a single round-trip instead of two separate queries.
    #[allow(clippy::too_many_arguments)]
    pub async fn add_message(
        &self,
        session_key: &str,
        id: uuid::Uuid,
        role: &str,
        content: &str,
        request_id: Option<&str>,
        tool_calls: Option<&serde_json::Value>,
        metadata: Option<&serde_json::Value>,
    ) -> Result<SessionMessageRow, StorageError> {
        let now = Utc::now();
        let row = sqlx::query_as::<_, SessionMessageRow>(
            "WITH touch AS (
                 UPDATE sessions SET updated_at = ?5 WHERE key = ?2
             )
             INSERT INTO session_messages
                 (id, session_key, role, content, timestamp, request_id, tool_calls, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             RETURNING *",
        )
        .bind(id)
        .bind(session_key)
        .bind(role)
        .bind(content)
        .bind(now)
        .bind(request_id)
        .bind(tool_calls)
        .bind(metadata)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Batch-insert multiple messages.
    ///
    /// Inserts each message individually using `INSERT OR IGNORE` to skip
    /// duplicates, and touches `sessions.updated_at` once before inserting.
    #[allow(clippy::too_many_arguments)]
    pub async fn batch_add_messages(
        &self,
        session_key: &str,
        ids: &[uuid::Uuid],
        roles: &[String],
        contents: &[String],
        timestamps: &[chrono::DateTime<Utc>],
        request_ids: &[Option<String>],
        tool_calls_list: &[Option<serde_json::Value>],
        metadata_list: &[Option<serde_json::Value>],
    ) -> Result<u64, StorageError> {
        if ids.is_empty() {
            return Ok(0);
        }
        let now = Utc::now();

        // Touch session updated_at once
        sqlx::query("UPDATE sessions SET updated_at = ?1 WHERE key = ?2")
            .bind(now)
            .bind(session_key)
            .execute(&self.pool)
            .await?;

        // Batch insert all messages in a single statement using QueryBuilder.
        // SQLite supports up to 999 bind parameters; each row uses 8 binds,
        // so we chunk into batches of 124 rows (992 binds) to stay under the limit.
        let mut inserted = 0u64;
        for chunk_start in (0..ids.len()).step_by(124) {
            let chunk_end = (chunk_start + 124).min(ids.len());
            let mut qb = sqlx::QueryBuilder::new(
                "INSERT OR IGNORE INTO session_messages \
                 (id, session_key, role, content, timestamp, request_id, tool_calls, metadata) ",
            );
            qb.push_values(chunk_start..chunk_end, |mut b, i| {
                b.push_bind(ids[i])
                    .push_bind(session_key)
                    .push_bind(&roles[i])
                    .push_bind(&contents[i])
                    .push_bind(timestamps[i])
                    .push_bind(request_ids[i].as_deref())
                    .push_bind(&tool_calls_list[i])
                    .push_bind(&metadata_list[i]);
            });
            let result = qb.build().execute(&self.pool).await?;
            inserted += result.rows_affected();
        }

        Ok(inserted)
    }

    /// Get all messages for a session, ordered by timestamp ascending.
    pub async fn get_messages(
        &self,
        session_key: &str,
    ) -> Result<Vec<SessionMessageRow>, StorageError> {
        let rows = sqlx::query_as::<_, SessionMessageRow>(
            "SELECT * FROM session_messages WHERE session_key = ?1 ORDER BY timestamp ASC",
        )
        .bind(session_key)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get the most recent N messages for a session.
    pub async fn get_recent_messages(
        &self,
        session_key: &str,
        limit: i64,
    ) -> Result<Vec<SessionMessageRow>, StorageError> {
        let rows = sqlx::query_as::<_, SessionMessageRow>(
            "SELECT * FROM (
                SELECT * FROM session_messages
                WHERE session_key = ?1
                ORDER BY timestamp DESC
                LIMIT ?2
             ) sub ORDER BY timestamp ASC",
        )
        .bind(session_key)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Count messages in a session.
    pub async fn count_messages(&self, session_key: &str) -> Result<i64, StorageError> {
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM session_messages WHERE session_key = ?1")
                .bind(session_key)
                .fetch_one(&self.pool)
                .await?;
        Ok(row.0)
    }

    /// Compact a session by keeping only the most recent `keep_count` messages.
    /// Returns the number of deleted rows.
    pub async fn compact_session(
        &self,
        session_key: &str,
        keep_count: i64,
    ) -> Result<u64, StorageError> {
        let result = sqlx::query(
            "DELETE FROM session_messages
             WHERE session_key = ?1
               AND id NOT IN (
                   SELECT id FROM session_messages
                   WHERE session_key = ?1
                   ORDER BY timestamp DESC
                   LIMIT ?2
               )",
        )
        .bind(session_key)
        .bind(keep_count)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Delete a session and all its messages (CASCADE).
    pub async fn delete_session(&self, key: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM sessions WHERE key = ?1")
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Update the tool_calls and metadata columns on the most recent assistant
    /// message for the given session key. Used by the dashboard WebSocket handler
    /// to persist tool-call and entity-card data after the agent saves the message.
    pub async fn update_last_assistant_metadata(
        &self,
        session_key: &str,
        tool_calls: Option<&serde_json::Value>,
        metadata: Option<&serde_json::Value>,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query(
            "UPDATE session_messages
             SET tool_calls = COALESCE(?2, tool_calls),
                 metadata   = COALESCE(?3, metadata)
             WHERE id = (
                 SELECT id FROM session_messages
                 WHERE session_key = ?1 AND role = 'assistant'
                 ORDER BY timestamp DESC
                 LIMIT 1
             )",
        )
        .bind(session_key)
        .bind(tool_calls)
        .bind(metadata)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Delete all sessions that have not been updated within the given TTL.
    ///
    /// Returns the number of sessions deleted (messages are cascade-deleted by the DB).
    pub async fn delete_stale_sessions(&self, ttl_days: u32) -> Result<u64, StorageError> {
        let cutoff = Utc::now() - chrono::Duration::days(ttl_days as i64);
        let result = sqlx::query("DELETE FROM sessions WHERE updated_at < ?1")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}
