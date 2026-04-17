use sqlx::SqlitePool;

use crate::error::StorageError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DndOverrideRow {
    pub id: i64,
    pub original_state: String,
    pub overridden_at: String,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DndOverrideRepo {
    pool: SqlitePool,
}

impl DndOverrideRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn set(
        &self,
        original_state: &str,
        session_id: Option<&str>,
    ) -> Result<(), StorageError> {
        let now = crate::sqlite_types::SqlTs(jiff::Timestamp::now());
        sqlx::query(
            "INSERT OR REPLACE INTO dnd_override (id, original_state, overridden_at, session_id)
             VALUES (1, ?1, ?2, ?3)",
        )
        .bind(original_state)
        .bind(&now)
        .bind(session_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn clear(&self) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM dnd_override WHERE id = 1")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get(&self) -> Result<Option<DndOverrideRow>, StorageError> {
        let row = sqlx::query_as::<_, DndOverrideRow>(
            "SELECT id, original_state, overridden_at, session_id FROM dnd_override WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }
}
