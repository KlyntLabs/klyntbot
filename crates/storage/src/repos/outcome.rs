//! Outcome repository — learning_outcomes table.

use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::learning::OutcomeRow;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ToolFailureStatsRow {
    pub tool_name: String,
    pub total_calls: i64,
    pub failure_count: i64,
    pub error_types: Option<String>,
}

/// Repository for learning outcome and enrichment feedback persistence.
#[derive(Debug, Clone)]
pub struct OutcomeRepo {
    pool: SqlitePool,
}

impl OutcomeRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a learning outcome record.
    pub async fn create(&self, row: &OutcomeRow) -> Result<OutcomeRow, StorageError> {
        let result = sqlx::query_as::<_, OutcomeRow>(
            "INSERT INTO learning_outcomes (id, session_key, tool_name, success, error_category,
                                            duration_ms, confidence_score, confidence_dimensions,
                                            execution_mode, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             RETURNING *",
        )
        .bind(&row.id)
        .bind(&row.session_key)
        .bind(&row.tool_name)
        .bind(row.success)
        .bind(&row.error_category)
        .bind(row.duration_ms)
        .bind(row.confidence_score)
        .bind(&row.confidence_dimensions)
        .bind(&row.execution_mode)
        .bind(row.created_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(result)
    }

    /// List outcomes within a date range.
    pub async fn list_by_date_range(
        &self,
        from: jiff::Timestamp,
        to: jiff::Timestamp,
    ) -> Result<Vec<OutcomeRow>, StorageError> {
        let rows = sqlx::query_as::<_, OutcomeRow>(
            "SELECT * FROM learning_outcomes
             WHERE created_at >= ?1 AND created_at <= ?2
             ORDER BY created_at DESC",
        )
        .bind(crate::sqlite_types::SqlTs::from(from))
        .bind(crate::sqlite_types::SqlTs::from(to))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// List outcomes for a specific tool.
    pub async fn list_by_tool(&self, tool_name: &str) -> Result<Vec<OutcomeRow>, StorageError> {
        let rows = sqlx::query_as::<_, OutcomeRow>(
            "SELECT * FROM learning_outcomes WHERE tool_name = ?1 ORDER BY created_at DESC",
        )
        .bind(tool_name)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Count outcomes (total, success) for aggregate stats.
    pub async fn count_stats(&self, since: jiff::Timestamp) -> Result<(i64, i64), StorageError> {
        let row: (i64, i64) = sqlx::query_as(
            "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN success THEN 1 ELSE 0 END), 0)
             FROM learning_outcomes WHERE created_at >= ?1",
        )
        .bind(crate::sqlite_types::SqlTs::from(since))
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Aggregate tool failure stats grouped by tool name since a timestamp.
    pub async fn tool_failure_stats_since(
        &self,
        since: jiff::Timestamp,
    ) -> Result<Vec<ToolFailureStatsRow>, StorageError> {
        let rows = sqlx::query_as::<_, ToolFailureStatsRow>(
            "SELECT tool_name,
                    COUNT(*) AS total_calls,
                    SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END) AS failure_count,
                    GROUP_CONCAT(DISTINCT CASE WHEN success = 0 THEN error_category ELSE NULL END) AS error_types
             FROM learning_outcomes
             WHERE created_at > ?1
             GROUP BY tool_name
             HAVING failure_count > 0
             ORDER BY failure_count DESC
             LIMIT 20",
        )
        .bind(since.to_string())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    delete_older_than_impl!("learning_outcomes", "created_at");
}
