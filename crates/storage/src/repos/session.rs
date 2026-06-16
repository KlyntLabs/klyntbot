//! Session repository — sessions + session_messages tables.

use sqlx::SqlitePool;

use crate::error::{OptionExt, StorageError};
use crate::messages::parts::{FinishReason, MessagePart};
use crate::rows::session::{SessionListRow, SessionMessageRow, SessionRow};

/// Concatenate Text parts with newline separators for the legacy `content`
/// column. Anthropic-spec providers reject rows whose `content` is empty,
/// so we keep this mirror in sync with `parts` on every write.
fn parts_to_content_text(parts: &[MessagePart]) -> String {
    parts
        .iter()
        .filter_map(|p| match p {
            MessagePart::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Deserialize an optional JSON column, treating NULL or empty string as `None`.
fn parse_json_column<T: serde::de::DeserializeOwned>(
    col: &Option<String>,
) -> Result<Option<T>, StorageError> {
    match col.as_deref() {
        Some(s) if !s.is_empty() => serde_json::from_str(s)
            .map_err(StorageError::serialization)
            .map(Some),
        _ => Ok(None),
    }
}

/// Repository for session and message persistence.
#[derive(Debug, Clone)]
pub struct SessionRepo {
    pool: SqlitePool,
}

impl SessionRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert or refresh a session with a known mode.
    /// On conflict the `mode` column is NOT overwritten — mode is set at
    /// creation time and is immutable for the life of the session.
    pub async fn upsert_session_with_mode(
        &self,
        key: &str,
        mode: common::SessionMode,
        metadata: &serde_json::Value,
    ) -> Result<SessionRow, StorageError> {
        let now = jiff::Timestamp::now().as_millisecond();
        let row = sqlx::query_as::<_, SessionRow>(
            "INSERT INTO sessions (key, mode, metadata, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)
             ON CONFLICT (key) DO UPDATE SET
               metadata   = excluded.metadata,
               updated_at = excluded.updated_at
             RETURNING *",
        )
        .bind(key)
        .bind(mode.as_str())
        .bind(metadata)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
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
            "INSERT INTO sessions (key, mode, metadata, created_at, updated_at)
             VALUES (?1, 'assistant', ?2, ?3, ?3)
             ON CONFLICT (key) DO UPDATE SET
               updated_at = ?3
             RETURNING *",
        )
        .bind(key)
        .bind(metadata)
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

    /// Fetch sessions by a list of keys. Missing keys are silently skipped.
    pub async fn get_sessions_by_keys(
        &self,
        keys: &[String],
    ) -> Result<Vec<SessionRow>, StorageError> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let mut qb =
            sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT * FROM sessions WHERE key IN (");
        let mut sep = qb.separated(", ");
        for key in keys {
            sep.push_bind(key);
        }
        qb.push(")");
        let rows = qb
            .build_query_as::<SessionRow>()
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// List recent sessions with message counts, ordered by updated_at descending.
    pub async fn list_recent(&self, limit: i64) -> Result<Vec<SessionListRow>, StorageError> {
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
             ORDER BY s.updated_at DESC
             LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
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

    /// List sessions filtered by mode (with message count).
    pub async fn list_sessions_by_mode(
        &self,
        mode: common::SessionMode,
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
             WHERE s.mode = ?1
             ORDER BY s.updated_at DESC",
        )
        .bind(mode.as_str())
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
        let mut tx = self.pool.begin().await?;
        let now: crate::sqlite_types::SqlTs = jiff::Timestamp::now().into();

        // Touch session updated_at
        sqlx::query("UPDATE sessions SET updated_at = ?1 WHERE key = ?2")
            .bind(now)
            .bind(session_key)
            .execute(&mut *tx)
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
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
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

    pub async fn count_user_messages(&self, session_key: &str) -> Result<i64, StorageError> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM session_messages WHERE session_key = ?1 AND role = 'user'",
        )
        .bind(session_key)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    /// Fetch the most recent assistant message in a given turn (by
    /// turn_id), returning its id and the concatenated Text content. Used
    /// by the coding bridge to coalesce streaming-snapshot + final-flush
    /// duplicates even when persistence happens from separate bridge
    /// tasks (each with its own in-process tracker).
    pub async fn latest_assistant_text_in_turn(
        &self,
        session_key: &str,
        turn_id: &str,
    ) -> Result<Option<(uuid::Uuid, String)>, StorageError> {
        let row: Option<SessionMessageRow> = sqlx::query_as(
            "SELECT * FROM session_messages \
             WHERE session_key = ?1 AND turn_id = ?2 AND role = 'assistant' \
             ORDER BY timestamp DESC, id DESC LIMIT 1",
        )
        .bind(session_key)
        .bind(turn_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(r) = row else {
            return Ok(None);
        };
        // Only consider rows whose payload is purely text — mixed payloads
        // (tool_call / tool_result / file_change) must never be coalesced
        // away.
        let parts: Option<Vec<MessagePart>> = parse_json_column(&r.parts)?;
        let text = match parts {
            Some(parts) => {
                let mut out = String::new();
                for p in parts {
                    match p {
                        MessagePart::Text { text } => out.push_str(&text),
                        MessagePart::Reasoning { .. } => {}
                        _ => return Ok(None),
                    }
                }
                out
            }
            None => r.content.clone(),
        };
        Ok(Some((r.id, text)))
    }

    /// Replace a message's `parts` (and the legacy `content` mirror) by id.
    ///
    /// Used by the coding turn-handler bridge to coalesce streaming-snapshot
    /// and final-flush duplicates: when a later flush's text is a superset of
    /// the earlier persisted row, we update that row in place rather than
    /// inserting a duplicate.
    ///
    /// Returns `true` when a row was actually updated.
    pub async fn update_message_parts(
        &self,
        message_id: uuid::Uuid,
        parts: &[MessagePart],
    ) -> Result<bool, StorageError> {
        let parts_json = serde_json::to_string(parts).map_err(StorageError::serialization)?;
        let content_text = parts_to_content_text(parts);
        let now: crate::sqlite_types::SqlTs = jiff::Timestamp::now().into();
        let result = sqlx::query(
            "UPDATE session_messages SET content = ?1, parts = ?2, timestamp = ?3 WHERE id = ?4",
        )
        .bind(&content_text)
        .bind(&parts_json)
        .bind(now)
        .bind(message_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Update the synthetic AGENTS.md message for a session.
    pub async fn update_synthetic_agents_md(
        &self,
        session_id: &str,
        new_body: &str,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query(
            "UPDATE session_messages
             SET content = ?1,
                 parts = ?2
             WHERE session_key = ?3 AND role = 'user'
               AND content LIKE '%AGENTS.md instructions for%'
             ORDER BY timestamp ASC
             LIMIT 1",
        )
        .bind(new_body)
        .bind(serde_json::json!([{"type":"text","text":new_body}]).to_string())
        .bind(session_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
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

    /// Best-effort fetch of the session title from metadata JSON.
    pub async fn get_title(&self, key: &str) -> Result<Option<String>, StorageError> {
        let row: Option<(Option<String>,)> =
            sqlx::query_as("SELECT json_extract(metadata, '$.title') FROM sessions WHERE key = ?")
                .bind(key)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.and_then(|(t,)| t))
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

    // ── Phase 4: Parts-aware methods ────────────────────────────────────

    /// Insert a message with typed `Vec<MessagePart>` content.
    ///
    /// The `content` column is set to empty string; the real content lives in `parts`.
    /// For legacy compatibility, callers that still use `content: String` should use
    /// `add_message` instead.
    #[allow(clippy::too_many_arguments)]
    pub async fn add_message_with_parts(
        &self,
        session_key: &str,
        message_id: uuid::Uuid,
        role: &str,
        parts: &[MessagePart],
        turn_id: Option<&str>,
        finish_reason: Option<&FinishReason>,
    ) -> Result<(), StorageError> {
        let mut tx = self.pool.begin().await?;
        let now: crate::sqlite_types::SqlTs = jiff::Timestamp::now().into();
        let parts_json = serde_json::to_string(parts).map_err(StorageError::serialization)?;
        let finish_json = finish_reason
            .map(serde_json::to_string)
            .transpose()
            .map_err(StorageError::serialization)?;
        // Mirror Text parts into the legacy `content` column — Anthropic-
        // spec providers 400 on empty content for non-tool messages.
        let content_text = parts_to_content_text(parts);

        // Touch session updated_at
        sqlx::query("UPDATE sessions SET updated_at = ?1 WHERE key = ?2")
            .bind(now)
            .bind(session_key)
            .execute(&mut *tx)
            .await?;

        sqlx::query(
            "INSERT INTO session_messages \
             (id, session_key, role, content, parts, turn_id, finish_reason, timestamp) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(message_id)
        .bind(session_key)
        .bind(role)
        .bind(&content_text)
        .bind(&parts_json)
        .bind(turn_id)
        .bind(finish_json)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Get messages for a session with Parts deserialized.
    ///
    /// Falls back to wrapping legacy `content` in a `Text` part when `parts` is NULL.
    pub async fn get_messages_parts(
        &self,
        session_key: &str,
        limit: i64,
    ) -> Result<Vec<SessionMessageWithParts>, StorageError> {
        let rows: Vec<SessionMessageRow> = sqlx::query_as(
            "SELECT * FROM session_messages \
             WHERE session_key = ?1 ORDER BY timestamp ASC LIMIT ?2",
        )
        .bind(session_key)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|r| {
                let parts: Vec<MessagePart> = parse_json_column(&r.parts)?.unwrap_or_else(|| {
                    vec![MessagePart::Text {
                        text: r.content.clone(),
                    }]
                });
                let finish_reason: Option<FinishReason> = parse_json_column(&r.finish_reason)?;
                Ok(SessionMessageWithParts {
                    id: r.id.to_string(),
                    session_key: r.session_key,
                    role: r.role,
                    parts,
                    turn_id: r.turn_id,
                    finish_reason,
                    timestamp: r.timestamp.into(),
                    metadata: r.metadata,
                })
            })
            .collect()
    }

    /// Set the workspace_id on a session.
    pub async fn set_workspace_id(
        &self,
        session_key: &str,
        workspace_id: &str,
    ) -> Result<(), StorageError> {
        let now: crate::sqlite_types::SqlTs = jiff::Timestamp::now().into();
        sqlx::query(
            "UPDATE sessions SET workspace_id = ?1, updated_at = ?2 WHERE key = ?3 AND (workspace_id IS NULL OR workspace_id != ?1)",
        )
        .bind(workspace_id)
        .bind(now)
        .bind(session_key)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Set the ephemeral flag on a session.
    pub async fn set_ephemeral(
        &self,
        session_key: &str,
        ephemeral: bool,
    ) -> Result<(), StorageError> {
        let val = if ephemeral { 1i64 } else { 0i64 };
        sqlx::query("UPDATE sessions SET ephemeral = ?1 WHERE key = ?2 AND ephemeral != ?1")
            .bind(val)
            .bind(session_key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Set the archived_at timestamp on a session.
    pub async fn archive(&self, session_key: &str) -> Result<(), StorageError> {
        let now: crate::sqlite_types::SqlTs = jiff::Timestamp::now().into();
        sqlx::query("UPDATE sessions SET archived_at = ?1, updated_at = ?1 WHERE key = ?2 AND archived_at IS NULL")
            .bind(now)
            .bind(session_key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Detect "zombie" sessions: sessions whose last message is from the user
    /// (meaning the agent never replied) and whose updated_at is older than
    /// the given threshold.
    pub async fn detect_zombie_sessions(
        &self,
        threshold_ms: i64,
    ) -> Result<Vec<SessionRow>, StorageError> {
        let cutoff = jiff::Timestamp::now().as_millisecond() - threshold_ms;
        let rows: Vec<SessionRow> = sqlx::query_as::<_, SessionRow>(
            "SELECT s.* FROM sessions s
             WHERE s.updated_at < ?1
               AND s.archived_at IS NULL
               AND (
                 SELECT role FROM session_messages
                 WHERE session_key = s.key
                 ORDER BY timestamp DESC
                 LIMIT 1
               ) = 'user'",
        )
        .bind(cutoff)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Insert a new `mode='subagent'` session with `parent_session_id` set.
    pub async fn insert_subagent_session(
        &self,
        session_key: &str,
        parent_session_id: &str,
        _workspace_path: &str,
    ) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            INSERT INTO sessions (key, mode, parent_session_id)
            VALUES (?1, 'subagent', ?2)
            "#,
        )
        .bind(session_key)
        .bind(parent_session_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Load conversation history for a session, ordered by timestamp ascending.
    /// Alias for `get_messages` with the name expected by the subagent runtime.
    pub async fn load_messages(
        &self,
        session_key: &str,
    ) -> Result<Vec<SessionMessageRow>, StorageError> {
        self.get_messages(session_key).await
    }
}

/// A message with deserialized Parts — returned by `get_messages_parts`.
#[derive(Debug, Clone)]
pub struct SessionMessageWithParts {
    pub id: String,
    pub session_key: String,
    pub role: String,
    pub parts: Vec<MessagePart>,
    pub turn_id: Option<String>,
    pub finish_reason: Option<FinishReason>,
    pub timestamp: jiff::Timestamp,
    pub metadata: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn upsert_session_defaults_to_assistant() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let repos = crate::Repos::from_pool(&pool);
        let row = repos
            .sessions
            .upsert_session("chat:xyz", &serde_json::json!({}))
            .await
            .unwrap();
        assert_eq!(row.mode, "assistant");
    }

    #[tokio::test]
    async fn mode_is_immutable_on_conflict() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let repos = crate::Repos::from_pool(&pool);
        repos
            .sessions
            .upsert_session_with_mode(
                "sub:k",
                common::SessionMode::Subagent,
                &serde_json::json!({"v": 1}),
            )
            .await
            .unwrap();
        // Re-upsert with assistant mode — mode column must stay "subagent".
        let row = repos
            .sessions
            .upsert_session_with_mode(
                "sub:k",
                common::SessionMode::Assistant,
                &serde_json::json!({"v": 2}),
            )
            .await
            .unwrap();
        assert_eq!(row.mode, "subagent");
    }
}
