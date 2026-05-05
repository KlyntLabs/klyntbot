use crate::error::StorageError;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CodingReviewRow {
    pub id: String,
    pub session_id: String,
    pub summary: String,
    pub issues_json: String,
    pub target: Option<String>,
    pub delivery: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct CodingReviewsRepo {
    pool: SqlitePool,
}

impl CodingReviewsRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, row: &CodingReviewRow) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO coding_reviews (id, session_id, summary, issues_json, target, delivery, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&row.id)
        .bind(&row.session_id)
        .bind(&row.summary)
        .bind(&row.issues_json)
        .bind(&row.target)
        .bind(&row.delivery)
        .bind(&row.created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_by_session(
        &self,
        session_id: &str,
        limit: u32,
    ) -> Result<Vec<CodingReviewRow>, StorageError> {
        let rows = sqlx::query_as::<_, CodingReviewRow>(
            "SELECT id, session_id, summary, issues_json, target, delivery, created_at
             FROM coding_reviews WHERE session_id = ? ORDER BY created_at DESC LIMIT ?",
        )
        .bind(session_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }
}
