//! Repository for the `learning_state` key-value table.

use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::learning::LearningStateRow;

/// Repository for learning state persistence (adaptive thresholds, etc.).
#[derive(Debug, Clone)]
pub struct LearningStateRepo {
    pool: SqlitePool,
}

impl LearningStateRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get a learning state value by key.
    pub async fn get(&self, key: &str) -> Result<Option<LearningStateRow>, StorageError> {
        let row =
            sqlx::query_as::<_, LearningStateRow>("SELECT * FROM learning_state WHERE key = ?1")
                .bind(key)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row)
    }

    /// Get the JSON value for a key, or None if not found.
    pub async fn get_value(&self, key: &str) -> Result<Option<serde_json::Value>, StorageError> {
        let row: Option<(serde_json::Value,)> =
            sqlx::query_as("SELECT value FROM learning_state WHERE key = ?1")
                .bind(key)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.0))
    }

    /// Upsert a learning state key-value pair.
    pub async fn set(
        &self,
        key: &str,
        value: &serde_json::Value,
    ) -> Result<LearningStateRow, StorageError> {
        let row = sqlx::query_as::<_, LearningStateRow>(
            "INSERT INTO learning_state (key, value)
             VALUES (?1, ?2)
             ON CONFLICT (key)
             DO UPDATE SET value = ?2, updated_at = (unixepoch('now') * 1000)
             RETURNING *",
        )
        .bind(key)
        .bind(value)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Get all learning state entries.
    pub async fn get_all(&self) -> Result<Vec<LearningStateRow>, StorageError> {
        let rows =
            sqlx::query_as::<_, LearningStateRow>("SELECT * FROM learning_state ORDER BY key")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows)
    }

    /// Delete a learning state entry by key.
    pub async fn delete(&self, key: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM learning_state WHERE key = ?1")
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
