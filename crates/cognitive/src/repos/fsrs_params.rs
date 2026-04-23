//! FsrsParamsRepo — typed write surface for the fsrs_parameters table.
//! The reader stays in flashcard.rs for now (it has tight coupling with
//! scheduling); v3 only adds the writer needed by autotuner promotion.

use common::Result;
use storage::StoragePool;

#[derive(Clone)]
pub struct FsrsParamsRepo {
    pool: StoragePool,
}

impl FsrsParamsRepo {
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }

    /// Update the `desired_retention` for the singleton `local` row.
    /// Caller is responsible for clamping to FSRS-valid range [0.7, 0.99].
    pub async fn update_desired_retention(&self, retention: f64) -> Result<()> {
        if !(0.7..=0.99).contains(&retention) {
            return Err(common::KlyntbotError::Storage(format!(
                "fsrs desired_retention out of range: {retention} (must be 0.7..=0.99)"
            )));
        }
        sqlx::query(
            "UPDATE fsrs_parameters SET desired_retention = ?, trained_at = datetime('now') WHERE id = 'local'",
        )
        .bind(retention)
        .execute(self.pool.inner())
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn writes_and_reads_back() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        // Apply cognitive migrations so the table + seed row exist.
        let migrations = crate::repos::cognitive_migrations();
        storage::StoragePool::run_feature_migrations(pool.inner(), &migrations)
            .await
            .unwrap();

        let repo = FsrsParamsRepo::new(pool.clone());
        repo.update_desired_retention(0.85).await.unwrap();

        let (_w, retention): (String, f64) = sqlx::query_as(
            "SELECT weights, desired_retention FROM fsrs_parameters WHERE id = 'local'",
        )
        .fetch_one(pool.inner())
        .await
        .unwrap();
        assert!((retention - 0.85).abs() < 1e-9);
    }

    #[tokio::test]
    async fn rejects_out_of_range() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let migrations = crate::repos::cognitive_migrations();
        storage::StoragePool::run_feature_migrations(pool.inner(), &migrations)
            .await
            .unwrap();

        let repo = FsrsParamsRepo::new(pool);
        assert!(repo.update_desired_retention(1.5).await.is_err());
        assert!(repo.update_desired_retention(0.5).await.is_err());
    }
}
