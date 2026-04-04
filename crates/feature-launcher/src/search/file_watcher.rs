use super::SearchSource;
use dashmap::DashMap;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

type SourceEntry = (PathBuf, Arc<dyn SearchSource>, Option<Duration>);

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
                .map(|w| (w.path, w.source, w.min_interval))
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

                for event in events {
                    if event.kind != DebouncedEventKind::Any {
                        continue;
                    }
                    let changed = &event.path;
                    for (watched_path, source, min_interval) in map_clone.iter() {
                        if changed.starts_with(watched_path) || changed == watched_path {
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
                                        break;
                                    }
                                }
                                cooldowns.insert(name, Instant::now());
                            }

                            let source = Arc::clone(source);
                            tracing::debug!(
                                "File change detected for {}, refreshing {}",
                                watched_path.display(),
                                source.name()
                            );
                            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                                handle.spawn(async move {
                                    source.refresh().await;
                                });
                            }
                            break;
                        }
                    }
                }
            },
        )?;

        for (path, source, _) in &*source_map {
            if let Err(e) = debouncer
                .watcher()
                .watch(path, notify::RecursiveMode::NonRecursive)
            {
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
