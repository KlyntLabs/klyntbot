use super::SearchSource;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

use super::CachedResult;

pub struct RefreshEntry {
    pub source: Arc<dyn SearchSource>,
    pub interval: Duration,
}

pub struct BackgroundRefresher {
    entries: Vec<(RefreshEntry, Instant)>,
    query_cache: Arc<DashMap<(&'static str, String), CachedResult>>,
    shutdown: CancellationToken,
    last_cache_eviction: Instant,
}

impl BackgroundRefresher {
    pub fn new(
        entries: Vec<RefreshEntry>,
        query_cache: Arc<DashMap<(&'static str, String), CachedResult>>,
        shutdown: CancellationToken,
    ) -> Self {
        let entries = entries
            .into_iter()
            .map(|e| {
                let initial = Instant::now() - e.interval;
                (e, initial)
            })
            .collect();

        Self {
            entries,
            query_cache,
            shutdown,
            last_cache_eviction: Instant::now(),
        }
    }

    pub async fn run(mut self) {
        let mut tick = tokio::time::interval(Duration::from_secs(1));

        loop {
            tokio::select! {
                _ = self.shutdown.cancelled() => {
                    tracing::info!("BackgroundRefresher shutting down");
                    break;
                }
                _ = tick.tick() => {
                    self.tick().await;
                }
            }
        }
    }

    async fn tick(&mut self) {
        let now = Instant::now();

        for (entry, last_refreshed) in &mut self.entries {
            if now.duration_since(*last_refreshed) >= entry.interval {
                let source = Arc::clone(&entry.source);
                tracing::debug!("Refreshing source: {}", source.name());
                tokio::spawn(async move {
                    source.refresh().await;
                });
                *last_refreshed = Instant::now();
            }
        }

        // Evict expired cache entries every 60s
        if now.duration_since(self.last_cache_eviction) >= Duration::from_secs(60) {
            // Max TTL across all sources is 5s; use 2x as generous eviction window
            self.query_cache
                .retain(|_, v| v.created_at.elapsed() < Duration::from_secs(10));
            self.last_cache_eviction = Instant::now();
        }
    }

    /// Spawn the refresher as a background task.
    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(self.run())
    }
}
