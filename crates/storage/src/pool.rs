//! Connection pool with automatic migration.

use crate::error::StorageError;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

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
        let pool = sqlx::pool::PoolOptions::<sqlx::Sqlite>::new()
            .max_connections(5)
            .connect(&url)
            .await?;
        sqlx::query("PRAGMA journal_mode=WAL;")
            .execute(&pool)
            .await?;
        sqlx::query("PRAGMA foreign_keys=ON;")
            .execute(&pool)
            .await?;
        sqlx::query("PRAGMA busy_timeout = 5000;")
            .execute(&pool)
            .await?;
        // ~2MB per connection instead of default ~8MB (single-user app).
        sqlx::query("PRAGMA cache_size = -2000;")
            .execute(&pool)
            .await?;
        // Prevent unbounded WAL file growth (default is 1000 pages ≈ 4MB).
        sqlx::query("PRAGMA wal_autocheckpoint = 1000;")
            .execute(&pool)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self(pool))
    }

    /// Wrap an existing `sqlx::SqlitePool` without running migrations.
    pub fn from_existing(pool: sqlx::SqlitePool) -> Self {
        Self(pool)
    }

    /// Create an in-memory SQLite pool with all migrations applied.
    ///
    /// Useful for tests and as a fallback when no `data_dir` is configured.
    pub async fn connect_in_memory() -> Result<Self, StorageError> {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await?;
        sqlx::query("PRAGMA foreign_keys=ON;")
            .execute(&pool)
            .await?;
        sqlx::query("PRAGMA busy_timeout = 5000;")
            .execute(&pool)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self(pool))
    }

    /// Run PRAGMA optimize to update SQLite query statistics.
    /// Best called on graceful shutdown.
    pub async fn optimize(&self) -> Result<(), StorageError> {
        sqlx::query("PRAGMA optimize;").execute(&self.0).await?;
        Ok(())
    }

    /// Access the inner `sqlx::SqlitePool`.
    pub fn inner(&self) -> &sqlx::SqlitePool {
        &self.0
    }

    /// Run feature-owned migrations that haven't been applied yet.
    ///
    /// Each migration and its tracking INSERT are wrapped in an explicit
    /// transaction so a crash between them cannot leave the DB in a state
    /// where the SQL ran but was not recorded.
    pub async fn run_feature_migrations(
        pool: &sqlx::SqlitePool,
        migrations: &[tools_core::FeatureMigration],
    ) -> Result<(), StorageError> {
        use std::collections::HashSet;
        let mut seen: HashSet<(String, i64)> = HashSet::new();
        for m in migrations {
            if !seen.insert((m.feature_name.clone(), m.version)) {
                panic!(
                    "duplicate migration version for feature {}: v{}",
                    m.feature_name, m.version
                );
            }
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
                let mut tx = pool.begin().await?;
                sqlx::query(&m.sql).execute(&mut *tx).await?;
                sqlx::query(
                    "INSERT INTO _feature_migrations (feature_name, version, description) VALUES (?1, ?2, ?3)",
                )
                .bind(&m.feature_name)
                .bind(m.version)
                .bind(&m.description)
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
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

impl StoragePool {
    /// Spawn a background task that polls `PRAGMA data_version` at the given
    /// interval. When the version changes between ticks (i.e. a different
    /// connection wrote since we last looked), publish
    /// `DomainEvent::DataVersionBumped` on `bus`. Returns a
    /// `CancellationToken` that the caller stores to keep the watcher alive
    /// and to stop it on shutdown.
    ///
    /// IMPORTANT: SQLite's `PRAGMA data_version` only advances when *other*
    /// connections commit. Holding a long-lived borrow on a single connection
    /// would mask all updates — that's why we use `fetch_one(&self.0)`
    /// (a sqlx pool) on every tick, which checks out a fresh connection.
    pub async fn start_data_version_watcher(
        &self,
        bus: Arc<bus::DomainEventBus>,
        interval: Duration,
    ) -> CancellationToken {
        let token = CancellationToken::new();
        let token_child = token.clone();
        let pool = self.clone();
        tokio::spawn(async move {
            let mut last = match read_data_version(&pool).await {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("data_version_watcher: initial read failed: {e}");
                    return;
                }
            };
            let mut ticker = tokio::time::interval(interval);
            // Skip the immediate first tick — interval fires once at start.
            ticker.tick().await;
            loop {
                tokio::select! {
                    _ = token_child.cancelled() => {
                        tracing::debug!("data_version_watcher: cancelled");
                        break;
                    }
                    _ = ticker.tick() => {
                        match read_data_version(&pool).await {
                            Ok(current) if current != last => {
                                bus.publish(bus::DomainEvent::DataVersionBumped {
                                    previous: last,
                                    current,
                                });
                                last = current;
                            }
                            Ok(_) => {} // no change
                            Err(e) => {
                                tracing::warn!("data_version_watcher: read failed: {e}");
                            }
                        }
                    }
                }
            }
        });
        token
    }
}

async fn read_data_version(pool: &StoragePool) -> sqlx::Result<u32> {
    // `PRAGMA data_version` returns one row, one column (an i64).
    let v: i64 = sqlx::query_scalar("PRAGMA data_version")
        .fetch_one(pool.inner())
        .await?;
    Ok(v as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tools_core::FeatureMigration;

    #[tokio::test]
    #[should_panic(expected = "duplicate migration version for feature")]
    async fn duplicate_version_panics() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let migs = vec![
            FeatureMigration {
                feature_name: "x".into(),
                version: 1,
                description: "a".into(),
                sql: "CREATE TABLE a(x INT);".into(),
            },
            FeatureMigration {
                feature_name: "x".into(),
                version: 1,
                description: "b".into(),
                sql: "CREATE TABLE b(x INT);".into(),
            },
        ];
        StoragePool::run_feature_migrations(pool.inner(), &migs)
            .await
            .unwrap();
    }
}
