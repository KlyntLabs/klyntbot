//! Connection pool with automatic migration.

use crate::error::StorageError;

/// Newtype wrapper around `sqlx::PgPool` with auto-migration on connect.
#[derive(Clone)]
pub struct StoragePool(sqlx::PgPool);

impl StoragePool {
    /// Connect to the database and run all pending migrations.
    pub async fn connect(database_url: &str) -> Result<Self, StorageError> {
        let pool = sqlx::PgPool::connect(database_url).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self(pool))
    }

    /// Create a lazy pool that defers connection until the first query.
    ///
    /// Useful for tests that construct repos but don't hit the database,
    /// and for dual-mode constructors where SQL may not be used.
    /// No migrations are run. Queries will fail at runtime if `database_url`
    /// is invalid or unreachable.
    pub fn connect_lazy(database_url: &str) -> Result<Self, StorageError> {
        let pool = sqlx::postgres::PgPoolOptions::new().connect_lazy(database_url)?;
        Ok(Self(pool))
    }

    /// Wrap an existing `sqlx::PgPool` without running migrations.
    ///
    /// Use this when you already have a connected pool (e.g., from a builder
    /// that received the pool as a dependency).
    pub fn from_existing(pool: sqlx::PgPool) -> Self {
        Self(pool)
    }

    /// Access the inner `sqlx::PgPool`.
    pub fn inner(&self) -> &sqlx::PgPool {
        &self.0
    }

    /// Run feature-owned migrations that haven't been applied yet.
    ///
    /// Each feature crate provides its own migrations via `FeaturePackage::migrations()`.
    /// This method checks which have already been applied (tracked in `_feature_migrations`)
    /// and runs any new ones.
    pub async fn run_feature_migrations(
        pool: &sqlx::PgPool,
        migrations: &[tools_core::FeatureMigration],
    ) -> Result<(), StorageError> {
        for m in migrations {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM _feature_migrations WHERE feature_name = $1 AND version = $2)",
            )
            .bind(&m.feature_name)
            .bind(m.version)
            .fetch_one(pool)
            .await?;

            if !exists {
                tracing::info!(
                    feature = %m.feature_name,
                    version = m.version,
                    description = %m.description,
                    "Running feature migration"
                );
                sqlx::query(&m.sql).execute(pool).await?;
                sqlx::query(
                    "INSERT INTO _feature_migrations (feature_name, version, description) VALUES ($1, $2, $3)",
                )
                .bind(&m.feature_name)
                .bind(m.version)
                .bind(&m.description)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }
}

impl std::fmt::Debug for StoragePool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoragePool").finish_non_exhaustive()
    }
}
