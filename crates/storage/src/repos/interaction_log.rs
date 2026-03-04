//! Repository for the `interaction_log` table.

use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::learning::InteractionLogRow;

/// Repository for interaction logging (raw data for pattern analysis).
#[derive(Debug, Clone)]
pub struct InteractionLogRepo {
    pool: SqlitePool,
}

impl InteractionLogRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create an interaction log entry.
    pub async fn create(
        &self,
        agent_name: &str,
        tool_names: &[&str],
        channel: &str,
        duration_ms: Option<i64>,
    ) -> Result<InteractionLogRow, StorageError> {
        let tools_json = serde_json::to_string(tool_names).unwrap_or_else(|_| "[]".to_string());
        let row = sqlx::query_as::<_, InteractionLogRow>(
            r#"
            INSERT INTO interaction_log (agent_name, tool_names, channel, duration_ms)
            VALUES (?1, ?2, ?3, ?4)
            RETURNING *
            "#,
        )
        .bind(agent_name)
        .bind(&tools_json)
        .bind(channel)
        .bind(duration_ms)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Create an interaction log entry with a specific timestamp.
    pub async fn create_with_timestamp(
        &self,
        agent_name: &str,
        tool_names: &[&str],
        channel: &str,
        duration_ms: Option<i64>,
        timestamp: &str,
    ) -> Result<InteractionLogRow, StorageError> {
        let tools_json = serde_json::to_string(tool_names).unwrap_or_else(|_| "[]".to_string());
        let row = sqlx::query_as::<_, InteractionLogRow>(
            r#"
            INSERT INTO interaction_log (timestamp, agent_name, tool_names, channel, duration_ms)
            VALUES (?1, ?2, ?3, ?4, ?5)
            RETURNING *
            "#,
        )
        .bind(timestamp)
        .bind(agent_name)
        .bind(&tools_json)
        .bind(channel)
        .bind(duration_ms)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// List recent interactions (newest first).
    pub async fn list_recent(&self, limit: i64) -> Result<Vec<InteractionLogRow>, StorageError> {
        let rows = sqlx::query_as::<_, InteractionLogRow>(
            "SELECT * FROM interaction_log ORDER BY timestamp DESC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Count interactions by agent name.
    pub async fn count_by_agent(&self) -> Result<Vec<(String, i64)>, StorageError> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT agent_name, COUNT(*) as cnt FROM interaction_log GROUP BY agent_name ORDER BY cnt DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Total interaction count.
    pub async fn count(&self) -> Result<i64, StorageError> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM interaction_log")
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoragePool;

    #[tokio::test]
    async fn test_create_and_list() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = InteractionLogRepo::new(pool.inner().clone());

        repo.create("task", &["task", "memory"], "telegram", Some(150))
            .await
            .unwrap();
        repo.create("finance", &["finance"], "discord", Some(200))
            .await
            .unwrap();

        let recent = repo.list_recent(10).await.unwrap();
        assert_eq!(recent.len(), 2);
    }

    #[tokio::test]
    async fn test_count_by_agent() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = InteractionLogRepo::new(pool.inner().clone());

        for _ in 0..5 {
            repo.create("task", &["task"], "telegram", Some(100))
                .await
                .unwrap();
        }
        for _ in 0..3 {
            repo.create("finance", &["finance"], "telegram", Some(100))
                .await
                .unwrap();
        }

        let counts = repo.count_by_agent().await.unwrap();
        assert_eq!(counts.len(), 2);
        assert_eq!(counts[0].0, "task");
        assert_eq!(counts[0].1, 5);
    }

    #[tokio::test]
    async fn test_create_with_timestamp() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = InteractionLogRepo::new(pool.inner().clone());

        repo.create_with_timestamp(
            "task",
            &["task"],
            "telegram",
            Some(100),
            "2026-03-02 10:00:00",
        )
        .await
        .unwrap();

        let recent = repo.list_recent(1).await.unwrap();
        assert_eq!(recent[0].timestamp, "2026-03-02 10:00:00");
    }
}
