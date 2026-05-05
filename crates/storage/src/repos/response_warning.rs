use crate::error::StorageError;
use sqlx::SqlitePool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ResponseWarningRow {
    pub id: i64,
    pub request_id: String,
    pub warning_type: String,
    pub detail: Option<String>,
    pub chat_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct ResponseWarningRepo {
    pool: SqlitePool,
}

impl ResponseWarningRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(
        &self,
        request_id: &str,
        warning_type: &str,
        detail: Option<&str>,
        chat_id: Option<&str>,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO response_warnings (request_id, warning_type, detail, chat_id)
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(request_id)
        .bind(warning_type)
        .bind(detail)
        .bind(chat_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn count_by_type_since(
        &self,
        since: &str,
    ) -> Result<Vec<(String, i64)>, StorageError> {
        Ok(sqlx::query_as(
            "SELECT warning_type, COUNT(*) FROM response_warnings
             WHERE created_at > ?1
             GROUP BY warning_type
             ORDER BY COUNT(*) DESC",
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn prune(&self, max_age_days: u32) -> Result<u64, StorageError> {
        let result =
            sqlx::query("DELETE FROM response_warnings WHERE created_at < datetime('now', ?1)")
                .bind(format!("-{max_age_days} days"))
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected())
    }
}
