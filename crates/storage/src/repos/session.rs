//! Session repository — sessions + session_messages tables.

use chrono::Utc;
use sqlx::PgPool;

use crate::error::StorageError;
use crate::rows::session::{SessionMessageRow, SessionRow};

/// Repository for session and message persistence.
#[derive(Debug, Clone)]
pub struct SessionRepo {
    pool: PgPool,
}

impl SessionRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new session (upsert — ignores conflict on existing key).
    pub async fn create_session(
        &self,
        key: &str,
        metadata: &serde_json::Value,
    ) -> Result<SessionRow, StorageError> {
        let now = Utc::now();
        let row = sqlx::query_as::<_, SessionRow>(
            "INSERT INTO sessions (key, metadata, created_at, updated_at)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (key) DO UPDATE SET updated_at = $4
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
        sqlx::query_as::<_, SessionRow>("SELECT * FROM sessions WHERE key = $1")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| StorageError::NotFound(format!("session '{}'", key)))
    }

    /// List all sessions ordered by updated_at descending.
    pub async fn list_sessions(&self) -> Result<Vec<SessionRow>, StorageError> {
        let rows =
            sqlx::query_as::<_, SessionRow>("SELECT * FROM sessions ORDER BY updated_at DESC")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows)
    }

    /// Add a message to a session.
    pub async fn add_message(
        &self,
        session_key: &str,
        id: uuid::Uuid,
        role: &str,
        content: &str,
        request_id: Option<&str>,
    ) -> Result<SessionMessageRow, StorageError> {
        let now = Utc::now();
        // Touch session updated_at
        sqlx::query("UPDATE sessions SET updated_at = $1 WHERE key = $2")
            .bind(now)
            .bind(session_key)
            .execute(&self.pool)
            .await?;

        let row = sqlx::query_as::<_, SessionMessageRow>(
            "INSERT INTO session_messages (id, session_key, role, content, timestamp, request_id)
             VALUES ($1, $2, $3, $4, $5, $6)
             RETURNING *",
        )
        .bind(id)
        .bind(session_key)
        .bind(role)
        .bind(content)
        .bind(now)
        .bind(request_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Get all messages for a session, ordered by timestamp ascending.
    pub async fn get_messages(
        &self,
        session_key: &str,
    ) -> Result<Vec<SessionMessageRow>, StorageError> {
        let rows = sqlx::query_as::<_, SessionMessageRow>(
            "SELECT * FROM session_messages WHERE session_key = $1 ORDER BY timestamp ASC",
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
                WHERE session_key = $1
                ORDER BY timestamp DESC
                LIMIT $2
             ) sub ORDER BY timestamp ASC",
        )
        .bind(session_key)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Compact a session by keeping only the most recent `keep_count` messages.
    pub async fn compact_session(
        &self,
        session_key: &str,
        keep_count: i64,
    ) -> Result<u64, StorageError> {
        let result = sqlx::query(
            "DELETE FROM session_messages
             WHERE session_key = $1
               AND id NOT IN (
                   SELECT id FROM session_messages
                   WHERE session_key = $1
                   ORDER BY timestamp DESC
                   LIMIT $2
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
        let result = sqlx::query("DELETE FROM sessions WHERE key = $1")
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
