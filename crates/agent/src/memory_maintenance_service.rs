//! Background service that periodically prunes old conversation embeddings.

use std::time::Duration;

use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use storage::ConvEmbeddingRepo;

/// Spawns a background task that deletes conversation embeddings older than
/// `max_age_days` every `interval_hours` hours. Shuts down gracefully when
/// the `CancellationToken` is cancelled.
pub struct MemoryMaintenanceService {
    repo: ConvEmbeddingRepo,
    max_age_days: u32,
    interval: Duration,
    token: CancellationToken,
}

impl MemoryMaintenanceService {
    pub fn new(
        repo: ConvEmbeddingRepo,
        max_age_days: u32,
        maintenance_interval_hours: u32,
        token: CancellationToken,
    ) -> Self {
        Self {
            repo,
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
                    match self.repo.delete_old_embeddings(self.max_age_days).await {
                        Ok(0) => {}
                        Ok(n) => info!(
                            deleted = n,
                            max_age_days = self.max_age_days,
                            "MemoryMaintenanceService: pruned old conversation embeddings"
                        ),
                        Err(e) => warn!(
                            error = %e,
                            "MemoryMaintenanceService: failed to prune old embeddings"
                        ),
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
