use super::SearchSource;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::CachedResult;

pub struct RefreshEntry {
    pub source: Arc<dyn SearchSource>,
    pub interval: Duration,
}

struct TrackedSource {
    entry: RefreshEntry,
    last_refreshed: Instant,
    /// Guard against concurrent refreshes — skip if previous is still running.
    in_flight: Option<JoinHandle<()>>,
}

pub struct BackgroundRefresher {
    sources: Vec<TrackedSource>,
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
        let sources = entries
            .into_iter()
            .map(|e| {
                let initial = Instant::now() - e.interval;
                TrackedSource {
                    entry: e,
                    last_refreshed: initial,
                    in_flight: None,
                }
            })
            .collect();

        Self {
            sources,
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

        for tracked in &mut self.sources {
            if now.duration_since(tracked.last_refreshed) >= tracked.entry.interval {
                // Skip if previous refresh is still running (backpressure)
                if let Some(handle) = &tracked.in_flight {
                    if !handle.is_finished() {
                        tracing::debug!(
                            "Skipping refresh for {} — previous still running",
                            tracked.entry.source.name()
                        );
                        continue;
                    }
                }

                let source = Arc::clone(&tracked.entry.source);
                tracing::debug!("Refreshing source: {}", source.name());
                tracked.in_flight = Some(tokio::spawn(async move {
                    source.refresh().await;
                }));
                tracked.last_refreshed = Instant::now();
            }
        }

        // Evict expired cache entries every 90s (typical session length)
        if now.duration_since(self.last_cache_eviction) >= Duration::from_secs(90) {
            // Max TTL across all sources is 8s; use 2x as generous eviction window
            self.query_cache
                .retain(|_, v| v.created_at.elapsed() < Duration::from_secs(16));
            self.last_cache_eviction = Instant::now();
        }
    }

    /// Spawn the refresher as a background task.
    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(self.run())
    }
}
