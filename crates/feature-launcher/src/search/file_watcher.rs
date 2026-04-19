use super::SearchSource;
use dashmap::DashMap;
use futures_util::future::BoxFuture;
use notify_debouncer_mini::{new_debouncer, DebouncedEvent, DebouncedEventKind};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

type SourceEntry = (
    PathBuf,
    bool,
    Arc<dyn SearchSource>,
    Option<Duration>,
    Option<Arc<dyn Fn(Vec<DebouncedEvent>) -> BoxFuture<'static, ()> + Send + Sync>>,
);

/// A file path to watch, the source to refresh on change, and an optional
/// minimum interval between refreshes (cooldown). Sources like browser history
/// sit on high-churn files — the cooldown prevents redundant work even when the
/// underlying file changes many times per minute.
pub struct WatchEntry {
    pub path: PathBuf,
    pub source: Arc<dyn SearchSource>,
    /// When `Some`, refreshes are skipped if the previous refresh was less than
    /// this duration ago. `None` means refresh on every debounced event.
    pub min_interval: Option<Duration>,
    /// Whether to watch subdirectories recursively.
    pub recursive: bool,
    /// Optional custom handler invoked instead of `source.refresh()`.
    /// Receives the batch of debounced events that triggered this watch entry.
    pub on_change: Option<Arc<dyn Fn(Vec<DebouncedEvent>) -> BoxFuture<'static, ()> + Send + Sync>>,
}

pub struct SourceFileWatcher {
    _debouncer: notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>,
}

impl SourceFileWatcher {
    pub fn start(watches: Vec<WatchEntry>) -> Result<Self, notify::Error> {
        let source_map: Arc<Vec<SourceEntry>> = Arc::new(
            watches
                .into_iter()
                .filter(|w| w.path.exists())
                .map(|w| (w.path, w.recursive, w.source, w.min_interval, w.on_change))
                .collect(),
        );

        if source_map.is_empty() {
            tracing::debug!("SourceFileWatcher: no valid paths to watch");
        }

        // Per-source cooldown tracking — shared with the callback closure.
        let last_refreshed: Arc<DashMap<&'static str, Instant>> = Arc::new(DashMap::new());

        let map_clone = Arc::clone(&source_map);
        let cooldowns = Arc::clone(&last_refreshed);

        let mut debouncer = new_debouncer(
            Duration::from_millis(500),
            move |events: Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>| {
                let events = match events {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::debug!("File watcher error: {e}");
                        return;
                    }
                };

                // Collect all Any events upfront (avoids re-filtering per entry)
                let any_events: Vec<_> = events
                    .into_iter()
                    .filter(|e| e.kind == DebouncedEventKind::Any)
                    .collect();

                for (watched_path, _recursive, source, min_interval, on_change) in map_clone.iter()
                {
                    let matched: Vec<_> = any_events
                        .iter()
                        .filter(|e| e.path.starts_with(watched_path) || &e.path == watched_path)
                        .cloned()
                        .collect();
                    if matched.is_empty() {
                        continue;
                    }

                    // Enforce per-source cooldown
                    if let Some(interval) = min_interval {
                        let name = source.name();
                        if let Some(last) = cooldowns.get(name) {
                            if last.elapsed() < *interval {
                                tracing::debug!(
                                    "Skipping refresh for {} (cooldown {:.0?} remaining)",
                                    name,
                                    *interval - last.elapsed(),
                                );
                                continue;
                            }
                        }
                        cooldowns.insert(source.name(), Instant::now());
                    }

                    tracing::debug!(
                        "File change detected for {}, refreshing {}",
                        watched_path.display(),
                        source.name()
                    );

                    if let Ok(handle) = tokio::runtime::Handle::try_current() {
                        match on_change {
                            Some(handler) => {
                                let fut = handler(matched);
                                handle.spawn(fut);
                            }
                            None => {
                                let source = Arc::clone(source);
                                handle.spawn(async move {
                                    source.refresh().await;
                                });
                            }
                        }
                    }
                }
            },
        )?;

        for (path, recursive, source, _, _) in &*source_map {
            let mode = if *recursive {
                notify::RecursiveMode::Recursive
            } else {
                notify::RecursiveMode::NonRecursive
            };
            if let Err(e) = debouncer.watcher().watch(path, mode) {
                tracing::warn!(
                    "Failed to watch {} for {}: {e}",
                    path.display(),
                    source.name()
                );
            } else {
                tracing::debug!("Watching {} for {} changes", path.display(), source.name());
            }
        }

        Ok(Self {
            _debouncer: debouncer,
        })
    }
}
