//! Repository for persisted Reforge suggestions — self-feedback from previous cycles.

use sqlx::SqlitePool;

use crate::StorageError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReforgeSuggestionRow {
    pub id: String,
    pub suggestion_type: String,
    pub content: String,
    pub reason: String,
    pub confidence: f64,
    pub cycle_run_at: String,
    pub acted_upon: bool,
    pub created_at: String,
}

pub struct ReforgeSuggestionRepo {
    pool: SqlitePool,
}

impl ReforgeSuggestionRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, row: &ReforgeSuggestionRow) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO reforge_suggestions (id, suggestion_type, content, reason, confidence, cycle_run_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&row.id)
        .bind(&row.suggestion_type)
        .bind(&row.content)
        .bind(&row.reason)
        .bind(row.confidence)
        .bind(&row.cycle_run_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Load recent suggestions not yet acted upon, for feeding back into the next cycle.
    pub async fn recent_unacted(
        &self,
        limit: u32,
    ) -> Result<Vec<ReforgeSuggestionRow>, StorageError> {
        let rows = sqlx::query_as::<_, ReforgeSuggestionRow>(
            "SELECT * FROM reforge_suggestions
             WHERE acted_upon = 0
             ORDER BY created_at DESC
             LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Mark a suggestion as acted upon.
    pub async fn mark_acted(&self, id: &str) -> Result<(), StorageError> {
        sqlx::query("UPDATE reforge_suggestions SET acted_upon = 1 WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    delete_older_than_impl!("reforge_suggestions", "created_at");
}
