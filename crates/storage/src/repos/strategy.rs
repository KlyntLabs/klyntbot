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

    /// Update user_satisfaction on the most recent strategy record for a chat.
    /// Returns true if a record was updated, false if no matching record found.
    pub async fn set_satisfaction_for_chat(
        &self,
        chat_id: &str,
        since: DateTime<Utc>,
        satisfaction: f32,
    ) -> Result<bool, StorageError> {
        // SQLite doesn't support UPDATE...ORDER BY...LIMIT in standard syntax.
        // Use a subquery to find the most recent record's ID.
        let result = sqlx::query(
            "UPDATE strategy_records SET user_satisfaction = ?1 \
             WHERE id = ( \
               SELECT id FROM strategy_records \
               WHERE chat_id = ?2 AND timestamp >= ?3 \
               ORDER BY timestamp DESC LIMIT 1 \
             )",
        )
        .bind(satisfaction)
        .bind(chat_id)
        .bind(since.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
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

    #[tokio::test]
    async fn test_set_satisfaction_for_chat() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let repo = StrategyRepo::new(pool.inner().clone());

        let now = chrono::Utc::now();

        // Create a record with chat_id
        let row = StrategyRecordRow {
            id: uuid::Uuid::new_v4(),
            timestamp: now,
            request_id: "req-sat".to_string(),
            predicted_strategy: "DirectResponse".to_string(),
            actual_strategy: "DirectResponse".to_string(),
            escalation_count: 0,
            iterations_used: 1,
            max_iterations: 1,
            success: true,
            user_satisfaction: None,
            response_time_ms: 500,
            chat_id: Some("tg:123".to_string()),
        };
        repo.create(&row).await.unwrap();

        // Set satisfaction
        let since = now - chrono::Duration::minutes(5);
        let updated = repo
            .set_satisfaction_for_chat("tg:123", since, 1.0)
            .await
            .unwrap();
        assert!(updated);

        // Verify
        let fetched = repo.get(row.id).await.unwrap();
        assert_eq!(fetched.user_satisfaction, Some(1.0));
    }

    #[tokio::test]
    async fn test_set_satisfaction_no_match_wrong_chat() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let repo = StrategyRepo::new(pool.inner().clone());

        let since = chrono::Utc::now() - chrono::Duration::minutes(5);
        let updated = repo
            .set_satisfaction_for_chat("nonexistent", since, 1.0)
            .await
            .unwrap();
        assert!(!updated);
    }

    #[tokio::test]
    async fn test_set_satisfaction_updates_most_recent_only() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let repo = StrategyRepo::new(pool.inner().clone());

        let now = chrono::Utc::now();

        // Create two records for the same chat — older and newer
        let older = StrategyRecordRow {
            id: uuid::Uuid::new_v4(),
            timestamp: now - chrono::Duration::seconds(30),
            request_id: "req-old".to_string(),
            predicted_strategy: "DirectResponse".to_string(),
            actual_strategy: "DirectResponse".to_string(),
            escalation_count: 0,
            iterations_used: 1,
            max_iterations: 1,
            success: true,
            user_satisfaction: None,
            response_time_ms: 200,
            chat_id: Some("tg:456".to_string()),
        };
        let newer = StrategyRecordRow {
            id: uuid::Uuid::new_v4(),
            timestamp: now,
            request_id: "req-new".to_string(),
            predicted_strategy: "ToolAssisted".to_string(),
            actual_strategy: "ToolAssisted".to_string(),
            escalation_count: 0,
            iterations_used: 3,
            max_iterations: 5,
            success: true,
            user_satisfaction: None,
            response_time_ms: 1500,
            chat_id: Some("tg:456".to_string()),
        };

        repo.create(&older).await.unwrap();
        repo.create(&newer).await.unwrap();

        // Set satisfaction — should only update the newer record
        let since = now - chrono::Duration::minutes(5);
        let updated = repo
            .set_satisfaction_for_chat("tg:456", since, 0.0)
            .await
            .unwrap();
        assert!(updated);

        // Verify: newer has satisfaction, older does not
        let newer_fetched = repo.get(newer.id).await.unwrap();
        assert_eq!(newer_fetched.user_satisfaction, Some(0.0));

        let older_fetched = repo.get(older.id).await.unwrap();
        assert_eq!(older_fetched.user_satisfaction, None);
    }
}
