use sqlx::SqlitePool;
use storage::StorageError;

use super::utc_now_str;

/// SQLite row for practice_sessions (maps 1:1 to table).
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct PracticeSessionRow {
    pub id: String,
    pub note_id: String,
    pub source_lang: String,
    pub target_lang: String,
    pub status: String,
    pub segments: String,
    pub current_index: i64,
    pub results: String,
    pub user_translation_doc: Option<String>,
    pub average_score: Option<f64>,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub updated_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct PracticeSessionRepo {
    pool: SqlitePool,
}

impl PracticeSessionRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a new practice session.
    pub async fn create(
        &self,
        row: &PracticeSessionRow,
    ) -> Result<PracticeSessionRow, StorageError> {
        let result = sqlx::query_as::<_, PracticeSessionRow>(
            "INSERT INTO practice_sessions
                (id, note_id, source_lang, target_lang, status, segments,
                 current_index, results, user_translation_doc, average_score,
                 started_at, completed_at, updated_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             RETURNING *",
        )
        .bind(&row.id)
        .bind(&row.note_id)
        .bind(&row.source_lang)
        .bind(&row.target_lang)
        .bind(&row.status)
        .bind(&row.segments)
        .bind(row.current_index)
        .bind(&row.results)
        .bind(&row.user_translation_doc)
        .bind(row.average_score)
        .bind(&row.started_at)
        .bind(&row.completed_at)
        .bind(&row.updated_at)
        .bind(&row.created_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(result)
    }

    /// Fetch a session by its primary key.
    pub async fn get_by_id(
        &self,
        id: &str,
    ) -> Result<Option<PracticeSessionRow>, StorageError> {
        let result =
            sqlx::query_as::<_, PracticeSessionRow>("SELECT * FROM practice_sessions WHERE id = ?1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(result)
    }

    /// Get the most recent in-progress session for a given note, if any.
    pub async fn get_active_for_note(
        &self,
        note_id: &str,
    ) -> Result<Option<PracticeSessionRow>, StorageError> {
        let result = sqlx::query_as::<_, PracticeSessionRow>(
            "SELECT * FROM practice_sessions
             WHERE note_id = ?1 AND status = 'in_progress'
             ORDER BY updated_at DESC
             LIMIT 1",
        )
        .bind(note_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(result)
    }

    /// Update current_index and results for an in-progress session.
    pub async fn update_progress(
        &self,
        id: &str,
        current_index: i64,
        results: &str,
        user_translation_doc: Option<&str>,
    ) -> Result<PracticeSessionRow, StorageError> {
        let result = sqlx::query_as::<_, PracticeSessionRow>(
            "UPDATE practice_sessions SET
                current_index = ?2,
                results = ?3,
                user_translation_doc = COALESCE(?4, user_translation_doc),
                updated_at = ?5
             WHERE id = ?1
             RETURNING *",
        )
        .bind(id)
        .bind(current_index)
        .bind(results)
        .bind(user_translation_doc)
        .bind(utc_now_str())
        .fetch_one(&self.pool)
        .await?;
        Ok(result)
    }

    /// Mark a session as completed with its final average score.
    pub async fn complete(
        &self,
        id: &str,
        average_score: f64,
        results: &str,
    ) -> Result<PracticeSessionRow, StorageError> {
        let now = utc_now_str();
        let result = sqlx::query_as::<_, PracticeSessionRow>(
            "UPDATE practice_sessions SET
                status = 'completed',
                average_score = ?2,
                results = ?3,
                completed_at = ?4,
                updated_at = ?4
             WHERE id = ?1
             RETURNING *",
        )
        .bind(id)
        .bind(average_score)
        .bind(results)
        .bind(&now)
        .fetch_one(&self.pool)
        .await?;
        Ok(result)
    }

    /// List all sessions for a note, most recent first.
    pub async fn list_for_note(
        &self,
        note_id: &str,
    ) -> Result<Vec<PracticeSessionRow>, StorageError> {
        let rows = sqlx::query_as::<_, PracticeSessionRow>(
            "SELECT * FROM practice_sessions
             WHERE note_id = ?1
             ORDER BY created_at DESC",
        )
        .bind(note_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Mark stale in-progress sessions (untouched for 7+ days) as abandoned.
    /// Returns the number of rows affected.
    pub async fn mark_abandoned_stale(&self) -> Result<u64, StorageError> {
        let result = sqlx::query(
            "UPDATE practice_sessions
             SET status = 'abandoned', updated_at = ?1
             WHERE status = 'in_progress'
               AND updated_at < strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-7 days')",
        )
        .bind(utc_now_str())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}
