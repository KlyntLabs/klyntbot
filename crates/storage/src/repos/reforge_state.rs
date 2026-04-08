//! Repository for the `reforge_state` singleton table.

use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::ReforgeStateRow;

#[derive(Debug, Clone)]
pub struct ReforgeStateRepo {
    pool: SqlitePool,
}

impl ReforgeStateRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get(&self) -> Result<ReforgeStateRow, StorageError> {
        let row = sqlx::query_as::<_, ReforgeStateRow>(
            "SELECT id, last_run_at, last_run_stats, run_count FROM reforge_state WHERE id = 'singleton'",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn record_run(&self, stats_json: &str) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE reforge_state SET
                last_run_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                last_run_stats = ?1,
                run_count = run_count + 1
             WHERE id = 'singleton'",
        )
        .bind(stats_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::StoragePool;

    #[tokio::test]
    async fn test_get_initial_state() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = ReforgeStateRepo::new(pool.inner().clone());
        let state = repo.get().await.unwrap();
        assert!(state.last_run_at.is_none());
        assert_eq!(state.run_count, 0);
    }

    #[tokio::test]
    async fn test_record_run_updates_state() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = ReforgeStateRepo::new(pool.inner().clone());
        repo.record_run(r#"{"facts_added": 3}"#).await.unwrap();
        let state = repo.get().await.unwrap();
        assert!(state.last_run_at.is_some());
        assert_eq!(state.run_count, 1);
        assert!(state.last_run_stats.unwrap().contains("facts_added"));
    }

    #[tokio::test]
    async fn test_record_run_increments_count() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = ReforgeStateRepo::new(pool.inner().clone());
        repo.record_run("{}").await.unwrap();
        repo.record_run("{}").await.unwrap();
        let state = repo.get().await.unwrap();
        assert_eq!(state.run_count, 2);
    }
}
