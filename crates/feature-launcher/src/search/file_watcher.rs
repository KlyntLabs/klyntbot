use super::SearchSource;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub struct SourceFileWatcher {
    _debouncer: notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>,
}

impl SourceFileWatcher {
    pub fn start(watches: Vec<(PathBuf, Arc<dyn SearchSource>)>) -> Result<Self, notify::Error> {
        let source_map: Arc<HashMap<PathBuf, Arc<dyn SearchSource>>> = Arc::new(
            watches
                .iter()
                .filter(|(path, _)| path.exists())
                .map(|(path, source)| (path.clone(), Arc::clone(source)))
                .collect(),
        );

        if source_map.is_empty() {
            tracing::debug!("SourceFileWatcher: no valid paths to watch");
        }

        let map_clone = Arc::clone(&source_map);
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
                    for (watched_path, source) in map_clone.iter() {
                        if changed.starts_with(watched_path) || changed == watched_path {
                            let source = Arc::clone(source);
                            tracing::info!(
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

        for (path, source) in &*source_map {
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
                tracing::info!("Watching {} for {} changes", path.display(), source.name());
            }
        }

        Ok(Self {
            _debouncer: debouncer,
        })
    }
}
