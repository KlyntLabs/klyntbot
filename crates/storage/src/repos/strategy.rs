//! Strategy repository — strategy_records table.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::{OptionExt, StorageError};
use crate::rows::learning::{StrategyRecordRow, StrategySummaryRow};

/// Repository for strategy execution record persistence.
#[derive(Debug, Clone)]
pub struct StrategyRepo {
    pool: SqlitePool,
}

/// Overall stats returned by get_overall_stats().
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverallStats {
    pub total_records: i64,
    pub accuracy: f64,
    pub avg_response_time_ms: i64,
    pub avg_satisfaction: Option<f64>,
}

/// Per-tool stats returned by get_tool_stats().
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatsRow {
    pub tool_name: String,
    pub total_calls: i64,
    pub success_count: i64,
    pub avg_duration_ms: i64,
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
                                           response_time_ms, chat_id,
                                           tool_name, tool_success, tool_duration_ms,
                                           complexity_signals, execution_mode)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
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
        .bind(&row.tool_name)
        .bind(row.tool_success)
        .bind(row.tool_duration_ms)
        .bind(&row.complexity_signals)
        .bind(&row.execution_mode)
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
            .ok_or_not_found(&format!("strategy record '{}'", id))
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

    /// Count total strategy records.
    pub async fn count_all(&self) -> Result<i64, StorageError> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM strategy_records")
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }

    /// Get overall stats: total records, accuracy, avg response time, avg satisfaction.
    pub async fn get_overall_stats(&self) -> Result<OverallStats, StorageError> {
        let row: (i64, i64, i64, Option<f64>) = sqlx::query_as(
            "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN predicted_strategy = actual_strategy THEN 1 ELSE 0 END), 0),
                    CAST(COALESCE(AVG(response_time_ms), 0) AS INTEGER),
                    AVG(user_satisfaction)
             FROM strategy_records",
        )
        .fetch_one(&self.pool)
        .await?;

        let accuracy = if row.0 > 0 {
            row.1 as f64 / row.0 as f64
        } else {
            0.0
        };

        Ok(OverallStats {
            total_records: row.0,
            accuracy,
            avg_response_time_ms: row.2,
            avg_satisfaction: row.3,
        })
    }

    /// Delete strategy records older than `days` days. Returns count of deleted rows.
    pub async fn delete_older_than(
        &self,
        days: i64,
        now: chrono::DateTime<Utc>,
    ) -> Result<u64, StorageError> {
        let cutoff = now - chrono::Duration::days(days);
        let result = sqlx::query("DELETE FROM strategy_records WHERE timestamp < ?1")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Get per-tool stats (only for records where tool_name is non-null).
    pub async fn get_tool_stats(&self) -> Result<Vec<ToolStatsRow>, StorageError> {
        let rows = sqlx::query_as::<_, ToolStatsRow>(
            "SELECT tool_name,
                    COUNT(*) AS total_calls,
                    COALESCE(SUM(CASE WHEN tool_success = 1 THEN 1 ELSE 0 END), 0) AS success_count,
                    CAST(COALESCE(AVG(tool_duration_ms), 0) AS INTEGER) AS avg_duration_ms
             FROM strategy_records
             WHERE tool_name IS NOT NULL
             GROUP BY tool_name
             ORDER BY total_calls DESC",
        )
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
            tool_name: None,
            tool_success: None,
            tool_duration_ms: None,
            complexity_signals: serde_json::Value::Null,
            execution_mode: None,
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
            tool_name: None,
            tool_success: None,
            tool_duration_ms: None,
            complexity_signals: serde_json::Value::Null,
            execution_mode: None,
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
            tool_name: None,
            tool_success: None,
            tool_duration_ms: None,
            complexity_signals: serde_json::Value::Null,
            execution_mode: None,
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
            tool_name: None,
            tool_success: None,
            tool_duration_ms: None,
            complexity_signals: serde_json::Value::Null,
            execution_mode: None,
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
            tool_name: None,
            tool_success: None,
            tool_duration_ms: None,
            complexity_signals: serde_json::Value::Null,
            execution_mode: None,
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

    #[tokio::test]
    async fn test_count_all() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let repo = StrategyRepo::new(pool.inner().clone());
        assert_eq!(repo.count_all().await.unwrap(), 0);

        let row = StrategyRecordRow {
            id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            request_id: "req-cnt".to_string(),
            predicted_strategy: "DirectResponse".to_string(),
            actual_strategy: "DirectResponse".to_string(),
            escalation_count: 0,
            iterations_used: 1,
            max_iterations: 1,
            success: true,
            user_satisfaction: None,
            response_time_ms: 100,
            chat_id: None,
            tool_name: None,
            tool_success: None,
            tool_duration_ms: None,
            complexity_signals: serde_json::Value::Null,
            execution_mode: None,
        };
        repo.create(&row).await.unwrap();
        assert_eq!(repo.count_all().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_get_overall_stats() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let repo = StrategyRepo::new(pool.inner().clone());

        let now = chrono::Utc::now();
        for (i, (pred, actual, sat)) in [
            ("DirectResponse", "DirectResponse", Some(1.0f32)),
            ("ToolAssisted", "ToolAssisted", None),
            ("DirectResponse", "ToolAssisted", Some(0.0f32)),
        ]
        .iter()
        .enumerate()
        {
            let row = StrategyRecordRow {
                id: uuid::Uuid::new_v4(),
                timestamp: now + chrono::Duration::seconds(i as i64),
                request_id: format!("req-{}", i),
                predicted_strategy: pred.to_string(),
                actual_strategy: actual.to_string(),
                escalation_count: 0,
                iterations_used: 1,
                max_iterations: 1,
                success: true,
                user_satisfaction: *sat,
                response_time_ms: 100 * (i as i64 + 1),
                chat_id: None,
                tool_name: None,
                tool_success: None,
                tool_duration_ms: None,
                complexity_signals: serde_json::Value::Null,
                execution_mode: None,
            };
            repo.create(&row).await.unwrap();
        }

        let stats = repo.get_overall_stats().await.unwrap();
        assert_eq!(stats.total_records, 3);
        assert!((stats.accuracy - 2.0 / 3.0).abs() < 0.01);
        assert_eq!(stats.avg_response_time_ms, 200);
        assert!((stats.avg_satisfaction.unwrap() - 0.5).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_get_tool_stats() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let repo = StrategyRepo::new(pool.inner().clone());

        for (tool, success) in [
            ("todo", true),
            ("todo", true),
            ("todo", false),
            ("shell", true),
        ] {
            let row = StrategyRecordRow {
                id: uuid::Uuid::new_v4(),
                timestamp: chrono::Utc::now(),
                request_id: uuid::Uuid::new_v4().to_string(),
                predicted_strategy: "ToolAssisted".to_string(),
                actual_strategy: "ToolAssisted".to_string(),
                escalation_count: 0,
                iterations_used: 1,
                max_iterations: 5,
                success: true,
                user_satisfaction: None,
                response_time_ms: 100,
                chat_id: None,
                tool_name: Some(tool.to_string()),
                tool_success: Some(success),
                tool_duration_ms: Some(50),
                complexity_signals: serde_json::Value::Null,
                execution_mode: None,
            };
            repo.create(&row).await.unwrap();
        }

        let stats = repo.get_tool_stats().await.unwrap();
        assert_eq!(stats.len(), 2);
        let todo = stats.iter().find(|s| s.tool_name == "todo").unwrap();
        assert_eq!(todo.total_calls, 3);
        assert_eq!(todo.success_count, 2);
    }
}
