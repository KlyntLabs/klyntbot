//! Repository for the `memory_notes` table.

use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::memory::MemoryNoteRow;

/// Repository for memory note persistence (daily notes + long-term memory).
#[derive(Debug, Clone)]
pub struct MemoryNoteRepo {
    pool: SqlitePool,
}

/// Well-known key for long-term memory (replaces MEMORY.md).
pub const LONG_TERM_KEY: &str = "LONG_TERM";

impl MemoryNoteRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get a memory note by key (date string or `LONG_TERM`).
    pub async fn get(&self, note_key: &str) -> Result<Option<MemoryNoteRow>, StorageError> {
        let row =
            sqlx::query_as::<_, MemoryNoteRow>("SELECT * FROM memory_notes WHERE note_key = ?1")
                .bind(note_key)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row)
    }

    /// Upsert a memory note (insert or update content).
    pub async fn upsert(
        &self,
        note_key: &str,
        content: &str,
    ) -> Result<MemoryNoteRow, StorageError> {
        let row = sqlx::query_as::<_, MemoryNoteRow>(
            r#"
            INSERT INTO memory_notes (note_key, content)
            VALUES (?1, ?2)
            ON CONFLICT (note_key)
            DO UPDATE SET content = ?2, updated_at = datetime('now')
            RETURNING *
            "#,
        )
        .bind(note_key)
        .bind(content)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Append content to an existing note (or create a new one).
    pub async fn append(
        &self,
        note_key: &str,
        content: &str,
    ) -> Result<MemoryNoteRow, StorageError> {
        let row = sqlx::query_as::<_, MemoryNoteRow>(
            r#"
            INSERT INTO memory_notes (note_key, content)
            VALUES (?1, ?2)
            ON CONFLICT (note_key)
            DO UPDATE SET
                content = CASE
                    WHEN memory_notes.content = '' THEN ?2
                    ELSE memory_notes.content || char(10) || char(10) || ?2
                END,
                updated_at = datetime('now')
            RETURNING *
            "#,
        )
        .bind(note_key)
        .bind(content)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// List recent daily notes (ordered newest-first, excludes LONG_TERM).
    pub async fn list_recent(&self, limit: i64) -> Result<Vec<MemoryNoteRow>, StorageError> {
        let rows = sqlx::query_as::<_, MemoryNoteRow>(
            r#"
            SELECT * FROM memory_notes
            WHERE note_key != ?2
            ORDER BY note_key DESC
            LIMIT ?1
            "#,
        )
        .bind(limit)
        .bind(LONG_TERM_KEY)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// List all memory note keys (newest-first).
    pub async fn list_keys(&self) -> Result<Vec<String>, StorageError> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT note_key FROM memory_notes ORDER BY note_key DESC")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    /// Search memory notes by content (case-insensitive).
    pub async fn search(&self, query: &str) -> Result<Vec<MemoryNoteRow>, StorageError> {
        let escaped = query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{escaped}%");
        let rows = sqlx::query_as::<_, MemoryNoteRow>(
            r#"
            SELECT * FROM memory_notes
            WHERE content LIKE ?1
            ORDER BY note_key DESC
            "#,
        )
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Delete a memory note by key.
    pub async fn delete(&self, note_key: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM memory_notes WHERE note_key = ?1")
            .bind(note_key)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
