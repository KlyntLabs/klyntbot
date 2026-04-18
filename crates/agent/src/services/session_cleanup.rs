//! Background service that periodically deletes stale sessions based on a configurable TTL.

use std::time::Duration;

use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use sqlx::SqlitePool;
use storage::SessionRepo;

/// Spawns a background task that deletes sessions older than `ttl_days` every
/// `interval_hours` hours. Shuts down gracefully when the `CancellationToken` is cancelled.
pub struct SessionCleanupService {
    repo: SessionRepo,
    pool: SqlitePool,
    ttl_days: u32,
    interval: Duration,
    token: CancellationToken,
}

impl SessionCleanupService {
    pub fn new(
        repo: SessionRepo,
        pool: SqlitePool,
        ttl_days: u32,
        cleanup_interval_hours: u32,
        token: CancellationToken,
    ) -> Self {
        Self {
            repo,
            pool,
            ttl_days,
            interval: Duration::from_secs(cleanup_interval_hours as u64 * 3600),
            token,
        }
    }

    /// Consume the service and spawn it as a background `tokio` task.
    pub fn spawn(self) {
        tokio::spawn(async move { self.run().await });
    }

    async fn run(self) {
        let mut interval = tokio::time::interval(self.interval);
        // Skip the first tick so we don't run immediately on startup.
        interval.tick().await;

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    match self.repo.delete_stale_sessions(self.ttl_days).await {
                        Ok(0) => {}
                        Ok(n) => info!(
                            deleted = n,
                            ttl_days = self.ttl_days,
                            "SessionCleanupService: deleted stale sessions"
                        ),
                        Err(e) => warn!(
                            error = %e,
                            "SessionCleanupService: failed to delete stale sessions"
                        ),
                    }

                    // Clean up old log records that grow without bound.
                    let cutoff = jiff::Timestamp::now()
                        .checked_sub(jiff::SignedDuration::from_secs(
                            self.ttl_days as i64 * 86_400,
                        ))
                        .unwrap_or_else(|_| jiff::Timestamp::now())
                        .to_string();
                    for (table, ts_col) in [
                        ("interaction_log", "timestamp"),
                        ("decision_log", "created_at"),
                        ("tool_usage", "created_at"),
                    ] {
                        match sqlx::query(&format!(
                            "DELETE FROM {table} WHERE {ts_col} < ?1"
                        ))
                        .bind(&cutoff)
                        .execute(&self.pool)
                        .await
                        {
                            Ok(result) => {
                                if result.rows_affected() > 0 {
                                    info!(
                                        table,
                                        deleted = result.rows_affected(),
                                        "cleaned old log records"
                                    );
                                }
                            }
                            Err(e) => {
                                // Tables may not exist yet — skip silently.
                                if !e.to_string().contains("no such table") {
                                    warn!(table, error = %e, "log cleanup failed");
                                }
                            }
                        }
                    }
                }
                _ = self.token.cancelled() => {
                    info!("SessionCleanupService: shutting down");
                    break;
                }
            }
        }
    }
}
