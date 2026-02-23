//! Connection pool with automatic migration.

use std::path::Path;
use crate::error::StorageError;

/// Newtype wrapper around `sqlx::SqlitePool` with auto-migration on connect.
#[derive(Clone)]
pub struct StoragePool(sqlx::SqlitePool);

impl StoragePool {
    /// Connect to (or create) the SQLite database at `{data_dir}/data.db`,
    /// enable WAL mode + foreign keys, and run all pending migrations.
    pub async fn connect(data_dir: &Path) -> Result<Self, StorageError> {
        let db_path = data_dir.join("data.db");
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                StorageError::Migration(format!("Failed to create data dir: {}", e))
            })?;
        }
        let url = format!("sqlite:{}?mode=rwc", db_path.display());
        let pool = sqlx::SqlitePool::connect(&url).await?;
        sqlx::query("PRAGMA journal_mode=WAL;").execute(&pool).await?;
        sqlx::query("PRAGMA foreign_keys=ON;").execute(&pool).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self(pool))
    }

    /// Wrap an existing `sqlx::SqlitePool` without running migrations.
    pub fn from_existing(pool: sqlx::SqlitePool) -> Self {
        Self(pool)
    }

    /// Access the inner `sqlx::SqlitePool`.
    pub fn inner(&self) -> &sqlx::SqlitePool {
        &self.0
    }

    /// Run feature-owned migrations that haven't been applied yet.
    pub async fn run_feature_migrations(
        pool: &sqlx::SqlitePool,
        migrations: &[tools_core::FeatureMigration],
    ) -> Result<(), StorageError> {
        for m in migrations {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM _feature_migrations WHERE feature_name = ?1 AND version = ?2)",
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
                    "INSERT INTO _feature_migrations (feature_name, version, description) VALUES (?1, ?2, ?3)",
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
