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

    /// Access the inner `sqlx::PgPool`.
    pub fn inner(&self) -> &sqlx::PgPool {
        &self.0
    }
}

impl std::fmt::Debug for StoragePool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoragePool").finish_non_exhaustive()
    }
}
