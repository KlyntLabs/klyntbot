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

    /// Exposed so callers that already hold an `Arc<FsrsParamsRepo>` can run
    /// bulk read queries (e.g. the FSRS weight optimiser scanning `review_log`)
    /// without the caller needing to keep a second pool reference.
    pub fn pool(&self) -> &StoragePool {
        &self.pool
    }

    /// Seconds since the singleton `local` row was last trained. Returns
    /// `None` when no prior training has occurred.
    pub async fn seconds_since_trained(&self) -> Result<Option<i64>> {
        let row: Option<(Option<String>,)> =
            sqlx::query_as("SELECT trained_at FROM fsrs_parameters WHERE id = 'local'")
                .fetch_optional(self.pool.inner())
                .await?;
        let Some((Some(ts),)) = row else {
            return Ok(None);
        };
        let trained: jiff::Timestamp = ts
            .parse()
            .map_err(|e| common::KlyntbotError::Storage(format!("fsrs trained_at parse: {e}")))?;
        let now = jiff::Timestamp::now();
        Ok(Some((now.as_second() - trained.as_second()).max(0)))
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

    /// Persist trained 19-weight FSRS-5 vector as a JSON array to the singleton
    /// `local` row. Caller is responsible for bounds-checking; this is a raw
    /// writer. See `cognitive::services::fsrs_optimizer::optimize_weights` for
    /// the training + validation path that gates persistence.
    pub async fn update_weights(&self, weights: &[f64; 19]) -> Result<()> {
        if weights.iter().any(|w| !w.is_finite()) {
            return Err(common::KlyntbotError::Storage(
                "fsrs weights contain NaN or infinity".to_string(),
            ));
        }
        let json = serde_json::to_string(weights)
            .map_err(|e| common::KlyntbotError::Storage(format!("fsrs weights serialise: {e}")))?;
        sqlx::query(
            "UPDATE fsrs_parameters SET weights = ?, trained_at = datetime('now') WHERE id = 'local'",
        )
        .bind(json)
        .execute(self.pool.inner())
        .await?;
        Ok(())
    }

    /// Read the current 19-weight FSRS-5 vector. Returns the migration-seeded
    /// defaults when the row is freshly inserted.
    pub async fn get_weights(&self) -> Result<[f64; 19]> {
        let (json,): (String,) =
            sqlx::query_as("SELECT weights FROM fsrs_parameters WHERE id = 'local'")
                .fetch_one(self.pool.inner())
                .await?;
        let vec: Vec<f64> = serde_json::from_str(&json).map_err(|e| {
            common::KlyntbotError::Storage(format!("fsrs weights deserialise: {e}"))
        })?;
        if vec.len() != 19 {
            return Err(common::KlyntbotError::Storage(format!(
                "fsrs weights: expected 19 values, got {}",
                vec.len()
            )));
        }
        let mut out = [0.0_f64; 19];
        out.copy_from_slice(&vec);
        Ok(out)
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
    async fn update_weights_round_trips() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let migrations = crate::repos::cognitive_migrations();
        StoragePool::run_feature_migrations(pool.inner(), &migrations)
            .await
            .unwrap();

        let repo = FsrsParamsRepo::new(pool.clone());
        let mut w = [0.0_f64; 19];
        for (i, slot) in w.iter_mut().enumerate() {
            *slot = 0.1 + i as f64;
        }
        repo.update_weights(&w).await.unwrap();
        let read_back = repo.get_weights().await.unwrap();
        for i in 0..19 {
            assert!((read_back[i] - w[i]).abs() < 1e-9, "slot {i}");
        }
    }

    #[tokio::test]
    async fn update_weights_rejects_non_finite() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let migrations = crate::repos::cognitive_migrations();
        StoragePool::run_feature_migrations(pool.inner(), &migrations)
            .await
            .unwrap();

        let repo = FsrsParamsRepo::new(pool);
        let mut w = [1.0_f64; 19];
        w[3] = f64::NAN;
        assert!(repo.update_weights(&w).await.is_err());
        w[3] = f64::INFINITY;
        assert!(repo.update_weights(&w).await.is_err());
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
