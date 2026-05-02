//! Session repository — sessions + session_messages tables.

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
    ///
    pub async fn upsert_session(
        &self,
        key: &str,
        metadata: &serde_json::Value,
    ) -> Result<SessionRow, StorageError> {
        let now: crate::sqlite_types::SqlTs = jiff::Timestamp::now().into();
        let row = sqlx::query_as::<_, SessionRow>(
            "INSERT INTO sessions (key, metadata, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (key) DO UPDATE SET
               updated_at = ?4
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

    /// Upsert a voice session — same as `upsert_session` but also sets
    /// `conversation_type = 'voice'` on insert and update.
    pub async fn upsert_voice_session(
        &self,
        key: &str,
        metadata: &serde_json::Value,
    ) -> Result<SessionRow, StorageError> {
        let now: crate::sqlite_types::SqlTs = jiff::Timestamp::now().into();
        let row = sqlx::query_as::<_, SessionRow>(
            "INSERT INTO sessions (key, metadata, conversation_type, created_at, updated_at)
             VALUES (?1, ?2, 'voice', ?3, ?4)
             ON CONFLICT (key) DO UPDATE SET
               updated_at = ?4,
               conversation_type = 'voice',
               metadata = ?2
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
                    COALESCE(counts.cnt, 0) AS message_count,
                    s.project_id, s.conversation_type, s.pinned
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

    /// Add a message to a session and touch its `updated_at` timestamp.
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
        let now: crate::sqlite_types::SqlTs = jiff::Timestamp::now().into();

        // Touch session updated_at
        sqlx::query("UPDATE sessions SET updated_at = ?1 WHERE key = ?2")
            .bind(now)
            .bind(session_key)
            .execute(&self.pool)
            .await?;

        // Insert the message
        let row = sqlx::query_as::<_, SessionMessageRow>(
            "INSERT INTO session_messages
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
        timestamps: &[jiff::Timestamp],
        request_ids: &[Option<String>],
        tool_calls_list: &[Option<serde_json::Value>],
        metadata_list: &[Option<serde_json::Value>],
    ) -> Result<u64, StorageError> {
        if ids.is_empty() {
            return Ok(0);
        }
        let now: crate::sqlite_types::SqlTs = jiff::Timestamp::now().into();

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
                    .push_bind(timestamps[i].as_millisecond())
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

    /// Rename a session by updating the title in its metadata JSON.
    pub async fn rename_session(&self, key: &str, new_title: &str) -> Result<bool, StorageError> {
        let now: crate::sqlite_types::SqlTs = jiff::Timestamp::now().into();
        let result = sqlx::query(
            "UPDATE sessions
             SET metadata = json_set(metadata, '$.title', ?2),
                 updated_at = ?3
             WHERE key = ?1",
        )
        .bind(key)
        .bind(new_title)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// List sessions updated since a cutoff date, ordered by updated_at descending.
    pub async fn list_sessions_since(
        &self,
        since: jiff::Timestamp,
    ) -> Result<Vec<SessionListRow>, StorageError> {
        let rows = sqlx::query_as::<_, SessionListRow>(
            "SELECT s.key, s.metadata, s.created_at, s.updated_at,
                    COALESCE(counts.cnt, 0) AS message_count,
                    s.project_id, s.conversation_type, s.pinned
             FROM sessions s
             LEFT JOIN (
                 SELECT session_key, COUNT(*) AS cnt
                 FROM session_messages
                 GROUP BY session_key
             ) counts ON counts.session_key = s.key
             WHERE s.updated_at >= ?1
             ORDER BY s.updated_at DESC",
        )
        .bind(crate::sqlite_types::SqlTs::from(since))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
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
    ///
    /// Prefer `update_assistant_metadata_by_id` when the message ID is known.
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
                 ORDER BY timestamp DESC, id DESC
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

    /// Update metadata on a specific assistant message by ID.
    /// This avoids the race condition of `update_last_assistant_metadata`
    /// when multiple assistant messages exist in the same session.
    pub async fn update_assistant_metadata_by_id(
        &self,
        message_id: &str,
        tool_calls: Option<&serde_json::Value>,
        metadata: Option<&serde_json::Value>,
    ) -> Result<bool, StorageError> {
        // IDs are stored as UUID binary blobs, so parse the string to uuid::Uuid
        // for correct binding. Fall back to text bind if parsing fails.
        let parsed = uuid::Uuid::parse_str(message_id);
        let result = if let Ok(ref uuid) = parsed {
            sqlx::query(
                "UPDATE session_messages
                 SET tool_calls = COALESCE(?2, tool_calls),
                     metadata   = COALESCE(?3, metadata)
                 WHERE id = ?1 AND role = 'assistant'",
            )
            .bind(uuid)
            .bind(tool_calls)
            .bind(metadata)
            .execute(&self.pool)
            .await?
        } else {
            sqlx::query(
                "UPDATE session_messages
                 SET tool_calls = COALESCE(?2, tool_calls),
                     metadata   = COALESCE(?3, metadata)
                 WHERE id = ?1 AND role = 'assistant'",
            )
            .bind(message_id)
            .bind(tool_calls)
            .bind(metadata)
            .execute(&self.pool)
            .await?
        };
        Ok(result.rows_affected() > 0)
    }

    /// Read the metadata JSON of a specific message by ID.
    pub async fn get_message_metadata_by_id(
        &self,
        message_id: &str,
    ) -> Result<Option<serde_json::Value>, StorageError> {
        let parsed = uuid::Uuid::parse_str(message_id);
        let row: Option<(Option<String>,)> = if let Ok(ref uuid) = parsed {
            sqlx::query_as("SELECT metadata FROM session_messages WHERE id = ?1")
                .bind(uuid)
                .fetch_optional(&self.pool)
                .await?
        } else {
            sqlx::query_as("SELECT metadata FROM session_messages WHERE id = ?1")
                .bind(message_id)
                .fetch_optional(&self.pool)
                .await?
        };
        Ok(row
            .and_then(|(meta,)| meta)
            .and_then(|s| serde_json::from_str(&s).ok()))
    }

    /// List sessions associated with a project, ordered by updated_at descending.
    pub async fn list_by_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<SessionListRow>, StorageError> {
        let rows = sqlx::query_as::<_, SessionListRow>(
            "SELECT s.key, s.metadata, s.created_at, s.updated_at,
                    COUNT(sm.id) AS message_count,
                    s.project_id, s.conversation_type, s.pinned
             FROM sessions s
             LEFT JOIN session_messages sm ON sm.session_key = s.key
             WHERE s.project_id = ?1
             GROUP BY s.key
             ORDER BY s.updated_at DESC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Delete all sessions whose key starts with `prefix`.
    ///
    /// Returns the number of sessions deleted. Messages are deleted via the
    /// `session_messages` table first (no FK cascade in SQLite by default).
    pub async fn delete_sessions_by_prefix(&self, prefix: &str) -> Result<u64, StorageError> {
        let pattern = format!("{prefix}%");
        // Delete messages first to avoid orphan rows.
        sqlx::query("DELETE FROM session_messages WHERE session_key LIKE ?1")
            .bind(&pattern)
            .execute(&self.pool)
            .await?;
        let result = sqlx::query("DELETE FROM sessions WHERE key LIKE ?1")
            .bind(&pattern)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Delete all sessions that have not been updated within the given TTL.
    ///
    /// Returns the number of sessions deleted (messages are cascade-deleted by the DB).
    pub async fn delete_stale_sessions(&self, ttl_days: u32) -> Result<u64, StorageError> {
        let cutoff =
            jiff::Timestamp::now() - jiff::SignedDuration::from_hours((ttl_days as i64) * 24);
        let result = sqlx::query("DELETE FROM sessions WHERE updated_at < ?1 AND pinned = 0")
            .bind(cutoff.as_millisecond())
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Save the compressed history prefix for a session.
    pub async fn save_compressed_prefix(
        &self,
        session_key: &str,
        prefix_json: &str,
        through_idx: i64,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE sessions SET compressed_prefix = ?1, compressed_through_idx = ?2, \
             compressed_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE key = ?3",
        )
        .bind(prefix_json)
        .bind(through_idx)
        .bind(session_key)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Load the compressed history prefix for a session.
    /// Returns (prefix_json, through_idx) or None if not set.
    pub async fn load_compressed_prefix(
        &self,
        session_key: &str,
    ) -> Result<Option<(String, i64)>, StorageError> {
        let row: Option<(Option<String>, Option<i64>)> = sqlx::query_as(
            "SELECT compressed_prefix, compressed_through_idx FROM sessions WHERE key = ?1",
        )
        .bind(session_key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.and_then(|(prefix, idx)| match (prefix, idx) {
            (Some(p), Some(i)) => Some((p, i)),
            _ => None,
        }))
    }

    /// Clear the compressed prefix (e.g., on message edit/delete).
    pub async fn clear_compressed_prefix(&self, session_key: &str) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE sessions SET compressed_prefix = NULL, compressed_through_idx = NULL, \
             compressed_at = NULL WHERE key = ?1",
        )
        .bind(session_key)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update the conversation type for a session.
    pub async fn update_conversation_type(&self, key: &str, t: &str) -> Result<(), StorageError> {
        sqlx::query("UPDATE sessions SET conversation_type = ?1 WHERE key = ?2")
            .bind(t)
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Update the approval mode for a session.
    pub async fn update_approval_mode(&self, key: &str, mode: &str) -> Result<(), StorageError> {
        sqlx::query("UPDATE sessions SET approval_mode = ?1 WHERE key = ?2")
            .bind(mode)
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn rewind_to_message(
        &self,
        session_key: &str,
        anchor_id: &str,
    ) -> Result<u64, StorageError> {
        let anchor_uuid = uuid::Uuid::parse_str(anchor_id)
            .map_err(|e| StorageError::NotFound(format!("invalid anchor uuid: {e}")))?;
        let res = sqlx::query(
            "DELETE FROM session_messages WHERE session_key = ? AND id IN ( \
                SELECT id FROM session_messages WHERE session_key = ? \
                  AND timestamp > (SELECT timestamp FROM session_messages WHERE id = ? AND session_key = ?) \
             )",
        )
        .bind(session_key).bind(session_key).bind(anchor_uuid).bind(session_key)
        .execute(&self.pool).await?;
        Ok(res.rows_affected())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn export_session_md(&self, session_key: &str) -> Result<String, StorageError> {
        let session = self.get_session(session_key).await?;
        let messages = self.get_messages(session_key).await?;
        let mut out = format!("# Session {}\n\n", session.key);
        for m in messages {
            out.push_str(&format!("### {}\n{}\n\n", m.role, m.content));
        }
        Ok(out)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn export_session_json(&self, session_key: &str) -> Result<String, StorageError> {
        let session = self.get_session(session_key).await?;
        let messages = self.get_messages(session_key).await?;
        let json = serde_json::json!({
            "session": {
                "key": session.key,
                "metadata": session.metadata,
                "created_at": session.created_at,
                "updated_at": session.updated_at,
            },
            "messages": messages.iter().map(|m| serde_json::json!({
                "id": m.id,
                "role": m.role,
                "content": m.content,
                "timestamp": m.timestamp,
                "request_id": m.request_id,
                "tool_calls": m.tool_calls,
                "metadata": m.metadata,
            })).collect::<Vec<_>>(),
        });
        Ok(json.to_string())
    }

    /// Decrement the `starred` counter and return sessions that should be pruned.
    #[tracing::instrument(skip(self), err)]
    pub async fn decrement_starred_prune(
        &self,
        ttl_days: i64,
    ) -> Result<Vec<String>, StorageError> {
        let cutoff = jiff::Timestamp::now().as_second() - (ttl_days * 86400);
        let rows = sqlx::query_scalar::<_, String>(
            "SELECT key FROM sessions \
             WHERE pinned = 0 AND updated_at < ?1 \
             AND key NOT IN (SELECT session_key FROM session_messages WHERE role = 'user' AND timestamp > ?1) \
             ORDER BY updated_at ASC LIMIT 100"
        )
        .bind(cutoff)
        .fetch_all(&self.pool).await?;
        Ok(rows)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn fork_session(
        &self,
        source_key: &str,
        up_to_message: Option<&str>,
    ) -> Result<String, StorageError> {
        let new_key = format!("fork-{}", uuid::Uuid::new_v4());
        let metadata = self.get_session(source_key).await?.metadata;
        sqlx::query(
            "INSERT INTO sessions (key, metadata, parent_session_id, conversation_type, approval_mode) \
             SELECT ?, ?, key, conversation_type, approval_mode FROM sessions WHERE key = ?"
        ).bind(&new_key).bind(&metadata).bind(source_key)
            .execute(&self.pool).await?;
        let cutoff_clause = match up_to_message {
            Some(_) => "AND timestamp <= (SELECT timestamp FROM session_messages WHERE id = ? AND session_key = ?)",
            None => "",
        };
        let q = format!(
            "INSERT INTO session_messages (session_key, id, role, content, timestamp, request_id, tool_calls, metadata) \
             SELECT ?, id || '-fork', role, content, timestamp, request_id, tool_calls, metadata \
             FROM session_messages WHERE session_key = ? {cutoff_clause} ORDER BY timestamp ASC"
        );
        let mut query = sqlx::query(&q).bind(&new_key).bind(source_key);
        if let Some(anchor) = up_to_message {
            query = query.bind(anchor).bind(source_key);
        }
        query.execute(&self.pool).await?;
        Ok(new_key)
    }
}
