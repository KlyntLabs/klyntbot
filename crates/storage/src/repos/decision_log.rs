//! Decision log repository — decision_log table.

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::learning::DecisionLogRow;

/// Repository for confidence decision log persistence.
#[derive(Debug, Clone)]
pub struct DecisionLogRepo {
    pool: SqlitePool,
}

impl DecisionLogRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a decision log entry.
    pub async fn create(&self, row: &DecisionLogRow) -> Result<DecisionLogRow, StorageError> {
        let result = sqlx::query_as::<_, DecisionLogRow>(
            "INSERT INTO decision_log (id, session_key, iteration, tool_names,
                                       user_message_preview, assessment, outcome, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             RETURNING *",
        )
        .bind(&row.id)
        .bind(&row.session_key)
        .bind(row.iteration)
        .bind(&row.tool_names)
        .bind(&row.user_message_preview)
        .bind(&row.assessment)
        .bind(&row.outcome)
        .bind(row.created_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(result)
    }

    /// List the most recent entries (newest first).
    pub async fn list_recent(&self, limit: i64) -> Result<Vec<DecisionLogRow>, StorageError> {
        let rows = sqlx::query_as::<_, DecisionLogRow>(
            "SELECT * FROM decision_log ORDER BY created_at DESC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// List entries within a date range.
    pub async fn list_by_date_range(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<DecisionLogRow>, StorageError> {
        let rows = sqlx::query_as::<_, DecisionLogRow>(
            "SELECT * FROM decision_log
             WHERE created_at >= ?1 AND created_at <= ?2
             ORDER BY created_at DESC",
        )
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
