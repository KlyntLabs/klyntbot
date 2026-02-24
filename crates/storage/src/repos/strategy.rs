//! Strategy repository — strategy_records table.

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::StorageError;
use crate::rows::learning::{StrategyRecordRow, StrategySummaryRow};

/// Repository for strategy execution record persistence.
#[derive(Debug, Clone)]
pub struct StrategyRepo {
    pool: SqlitePool,
}

impl StrategyRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a strategy record.
    pub async fn create(&self, row: &StrategyRecordRow) -> Result<StrategyRecordRow, StorageError> {
        let result = sqlx::query_as::<_, StrategyRecordRow>(
            "INSERT INTO strategy_records (id, timestamp, request_id, predicted_strategy,
                                           actual_strategy, escalation_count, iterations_used,
                                           max_iterations, success, user_satisfaction,
                                           response_time_ms, chat_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             RETURNING *",
        )
        .bind(row.id)
        .bind(row.timestamp)
        .bind(&row.request_id)
        .bind(&row.predicted_strategy)
        .bind(&row.actual_strategy)
        .bind(row.escalation_count)
        .bind(row.iterations_used)
        .bind(row.max_iterations)
        .bind(row.success)
        .bind(row.user_satisfaction)
        .bind(row.response_time_ms)
        .bind(&row.chat_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(result)
    }

    /// Get a strategy record by ID.
    pub async fn get(&self, id: Uuid) -> Result<StrategyRecordRow, StorageError> {
        sqlx::query_as::<_, StrategyRecordRow>("SELECT * FROM strategy_records WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| StorageError::NotFound(format!("strategy record '{}'", id)))
    }

    /// List strategy records by predicted strategy within a date range.
    pub async fn list_by_strategy(
        &self,
        strategy: &str,
        since: DateTime<Utc>,
    ) -> Result<Vec<StrategyRecordRow>, StorageError> {
        let rows = sqlx::query_as::<_, StrategyRecordRow>(
            "SELECT * FROM strategy_records
             WHERE predicted_strategy = ?1 AND timestamp >= ?2
             ORDER BY timestamp DESC",
        )
        .bind(strategy)
        .bind(since)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get accuracy for a strategy (fraction where predicted == actual) over a date range.
    pub async fn get_accuracy(
        &self,
        strategy: &str,
        since: DateTime<Utc>,
    ) -> Result<Option<f32>, StorageError> {
        let row: (i64, i64) = sqlx::query_as(
            "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN predicted_strategy = actual_strategy THEN 1 ELSE 0 END), 0)
             FROM strategy_records
             WHERE predicted_strategy = ?1 AND timestamp >= ?2",
        )
        .bind(strategy)
        .bind(since)
        .fetch_one(&self.pool)
        .await?;

        if row.0 == 0 {
            Ok(None)
        } else {
            Ok(Some(row.1 as f32 / row.0 as f32))
        }
    }

    /// Get aggregated strategy performance summaries since a given date.
    ///
    /// Returns per-strategy accuracy, sample count, and average escalations.
    pub async fn get_strategy_summaries(
        &self,
        since: DateTime<Utc>,
    ) -> Result<Vec<StrategySummaryRow>, StorageError> {
        let rows = sqlx::query_as::<_, StrategySummaryRow>(
            "SELECT predicted_strategy,
                    COUNT(*) AS sample_count,
                    COALESCE(SUM(CASE WHEN predicted_strategy = actual_strategy THEN 1 ELSE 0 END), 0)
                        AS correct_count,
                    AVG(CAST(escalation_count AS REAL)) AS avg_escalations
             FROM strategy_records
             WHERE timestamp >= ?1
             GROUP BY predicted_strategy
             ORDER BY sample_count DESC",
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// List all records within a date range.
    pub async fn list_by_date_range(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<StrategyRecordRow>, StorageError> {
        let rows = sqlx::query_as::<_, StrategyRecordRow>(
            "SELECT * FROM strategy_records
             WHERE timestamp >= ?1 AND timestamp <= ?2
             ORDER BY timestamp DESC",
        )
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_strategy_record_with_chat_id() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let repo = StrategyRepo::new(pool.inner().clone());

        let row = StrategyRecordRow {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            request_id: "req-1".to_string(),
            predicted_strategy: "DirectResponse".to_string(),
            actual_strategy: "ToolAssisted".to_string(),
            escalation_count: 1,
            iterations_used: 3,
            max_iterations: 5,
            success: true,
            user_satisfaction: None,
            response_time_ms: 1200,
            chat_id: Some("tg:12345".to_string()),
        };

        let created = repo.create(&row).await.unwrap();
        assert_eq!(created.chat_id, Some("tg:12345".to_string()));

        let fetched = repo.get(row.id).await.unwrap();
        assert_eq!(fetched.chat_id, Some("tg:12345".to_string()));
    }

    #[tokio::test]
    async fn test_create_strategy_record_without_chat_id() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let repo = StrategyRepo::new(pool.inner().clone());

        let row = StrategyRecordRow {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            request_id: "req-2".to_string(),
            predicted_strategy: "DirectResponse".to_string(),
            actual_strategy: "DirectResponse".to_string(),
            escalation_count: 0,
            iterations_used: 1,
            max_iterations: 1,
            success: true,
            user_satisfaction: None,
            response_time_ms: 200,
            chat_id: None,
        };

        let created = repo.create(&row).await.unwrap();
        assert_eq!(created.chat_id, None);
    }
}
