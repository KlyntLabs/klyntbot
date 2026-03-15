use chrono::Utc;
use sqlx::SqlitePool;
use storage::StorageError;

#[derive(Debug, Clone)]
pub struct FrequencyRepo {
    pool: SqlitePool,
}

impl FrequencyRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn increment(&self, item_id: &str, kind: &str) -> Result<(), StorageError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO launcher_frequencies (item_id, kind, count, last_used) \
             VALUES (?, ?, 1, ?) \
             ON CONFLICT(item_id, kind) DO UPDATE SET count = count + 1, last_used = ?",
        )
        .bind(item_id)
        .bind(kind)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_count(&self, item_id: &str, kind: &str) -> Result<i64, StorageError> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT count FROM launcher_frequencies WHERE item_id = ? AND kind = ?")
                .bind(item_id)
                .bind(kind)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map_or(0, |r| r.0))
    }

    /// Returns log2(count + 1) as a frequency boost multiplier
    pub async fn get_boost(&self, item_id: &str, kind: &str) -> Result<f64, StorageError> {
        let count = self.get_count(item_id, kind).await?;
        Ok((count as f64 + 1.0).log2())
    }

    pub async fn get_boosts_batch(
        &self,
        items: &[(String, String)],
    ) -> Result<Vec<f64>, StorageError> {
        let mut boosts = Vec::with_capacity(items.len());
        for (item_id, kind) in items {
            boosts.push(self.get_boost(item_id, kind).await?);
        }
        Ok(boosts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    async fn setup() -> FrequencyRepo {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(
            pool.inner(),
            &crate::LauncherFeature::migrations_static(),
        )
        .await
        .unwrap();
        FrequencyRepo::new(pool.inner().clone())
    }

    #[tokio::test]
    async fn test_increment_and_get() {
        let repo = setup().await;
        repo.increment("com.apple.Safari", "app").await.unwrap();
        repo.increment("com.apple.Safari", "app").await.unwrap();
        let count = repo.get_count("com.apple.Safari", "app").await.unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_get_nonexistent_returns_zero() {
        let repo = setup().await;
        let count = repo.get_count("nonexistent", "app").await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_boost_calculation() {
        let repo = setup().await;
        for _ in 0..10 {
            repo.increment("frequent", "app").await.unwrap();
        }
        repo.increment("rare", "app").await.unwrap();
        let frequent_boost = repo.get_boost("frequent", "app").await.unwrap();
        let rare_boost = repo.get_boost("rare", "app").await.unwrap();
        assert!(frequent_boost > rare_boost);
    }
}
