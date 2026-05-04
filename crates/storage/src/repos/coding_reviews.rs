use crate::pool::StoragePool;
use common::Result;
use serde::{Deserialize, Serialize};
use sqlx::Row;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pool: StoragePool,
}

impl CodingReviewsRepo {
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, row: &CodingReviewRow) -> Result<()> {
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
        .execute(self.pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn get_by_session(
        &self,
        session_id: &str,
        limit: u32,
    ) -> Result<Vec<CodingReviewRow>> {
        let rows = sqlx::query(
            "SELECT id, session_id, summary, issues_json, target, delivery, created_at
             FROM coding_reviews WHERE session_id = ? ORDER BY created_at DESC LIMIT ?",
        )
        .bind(session_id)
        .bind(limit as i64)
        .fetch_all(self.pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|r| CodingReviewRow {
                id: r.get(0),
                session_id: r.get(1),
                summary: r.get(2),
                issues_json: r.get(3),
                target: r.get(4),
                delivery: r.get(5),
                created_at: r.get(6),
            })
            .collect())
    }
}
