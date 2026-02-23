//! Background service that periodically prunes old conversation embeddings.

use std::time::Duration;

use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use storage::VectorStore;

/// Spawns a background task that deletes conversation embeddings older than
/// `max_age_days` every `interval_hours` hours. Shuts down gracefully when
/// the `CancellationToken` is cancelled.
pub struct MemoryMaintenanceService {
    store: VectorStore,
    max_age_days: u32,
    interval: Duration,
    token: CancellationToken,
}

impl MemoryMaintenanceService {
    pub fn new(
        store: VectorStore,
        max_age_days: u32,
        maintenance_interval_hours: u32,
        token: CancellationToken,
    ) -> Self {
        Self {
            store,
            max_age_days,
            interval: Duration::from_secs(maintenance_interval_hours as u64 * 3600),
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
                    let cutoff = chrono::Utc::now()
                        - chrono::Duration::days(self.max_age_days as i64);
                    let cutoff_str = cutoff.to_rfc3339();
                    let predicate = format!("created_at < '{cutoff_str}'");

                    let before = match self.store.count("conv_embeddings").await {
                        Ok(n) => n,
                        Err(e) => {
                            warn!(
                                error = %e,
                                "MemoryMaintenanceService: failed to count embeddings"
                            );
                            continue;
                        }
                    };

                    if let Err(e) = self.store
                        .delete_where("conv_embeddings", &predicate)
                        .await
                    {
                        warn!(
                            error = %e,
                            "MemoryMaintenanceService: failed to prune old embeddings"
                        );
                        continue;
                    }

                    let after = self.store.count("conv_embeddings").await.unwrap_or(before);
                    let deleted = before.saturating_sub(after);
                    if deleted > 0 {
                        info!(
                            deleted,
                            max_age_days = self.max_age_days,
                            "MemoryMaintenanceService: pruned old conversation embeddings"
                        );
                    }
                }
                _ = self.token.cancelled() => {
                    info!("MemoryMaintenanceService: shutting down");
                    break;
                }
            }
        }
    }
}
