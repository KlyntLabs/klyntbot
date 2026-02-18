//! ConfigWatcher — file system watcher for hot-reload of config.json.
//!
//! Uses the `notify` crate with a debounce layer to avoid emitting many
//! events when editors write files (most write several times per save).
//!
//! On file change:
//!   1. Read and validate the file as valid JSON
//!   2. Emit `DashboardEvent::ConfigReloaded` on the event bus

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use notify::event::{EventKind, ModifyKind};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::events::{DashboardEvent, DashboardEventBus};

/// Watches a config file and emits `ConfigReloaded` events on change.
pub struct ConfigWatcher {
    config_path: PathBuf,
    event_bus: Arc<DashboardEventBus>,
    /// Minimum time between events (debounce window).
    debounce: Duration,
}

impl ConfigWatcher {
    /// Create a new watcher.
    ///
    /// `config_path` — path to the file being watched
    /// `event_bus` — bus to emit `ConfigReloaded` events on
    /// `debounce` — debounce window (default: 200ms)
    pub fn new(
        config_path: PathBuf,
        event_bus: Arc<DashboardEventBus>,
        debounce: Duration,
    ) -> Self {
        Self {
            config_path,
            event_bus,
            debounce,
        }
    }

    /// Start watching the config file in a background task.
    ///
    /// Returns immediately; the watcher runs until the returned
    /// `tokio::task::JoinHandle` is dropped or aborted.
    pub fn start(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            if let Err(e) = self.run().await {
                warn!("ConfigWatcher stopped with error: {}", e);
            }
        })
    }

    async fn run(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (tx, mut rx) = mpsc::channel(32);

        // Canonicalize the config path so OS-level symlinks (e.g. /var → /private/var
        // on macOS) don't cause the path comparison to fail when notify resolves them.
        let config_path = self
            .config_path
            .canonicalize()
            .unwrap_or_else(|_| self.config_path.clone());

        // Watcher runs on a blocking thread (notify uses OS file system events).
        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
                    let _ = tx.blocking_send(event);
                }
            },
            notify::Config::default(),
        )?;

        // Watch the parent directory so we catch atomic writes (rename-based saves).
        let watch_dir = config_path.parent().ok_or("config path has no parent")?;

        watcher.watch(watch_dir, RecursiveMode::NonRecursive)?;
        debug!(
            "ConfigWatcher: watching {:?} for changes to {:?}",
            watch_dir, config_path
        );

        let debounce = self.debounce;
        // None means "never emitted" — always allow the first event through.
        let mut last_emit: Option<std::time::Instant> = None;

        while let Some(event) = rx.recv().await {
            // Only care about modify/create events on our specific file.
            // Canonicalize event paths to handle symlinks (e.g. /var → /private/var on macOS).
            let is_our_file = event
                .paths
                .iter()
                .any(|p| p.canonicalize().unwrap_or_else(|_| p.clone()) == config_path);
            if !is_our_file {
                continue;
            }

            let is_write = matches!(
                event.kind,
                EventKind::Modify(ModifyKind::Data(_))
                    | EventKind::Modify(ModifyKind::Any)
                    | EventKind::Create(_)
                    | EventKind::Modify(ModifyKind::Name(_))
            );
            if !is_write {
                continue;
            }

            // Debounce: only emit if enough time has passed since the last emit.
            let now = std::time::Instant::now();
            if let Some(last) = last_emit {
                if now.duration_since(last) < debounce {
                    continue;
                }
            }

            // Validate: must be valid JSON.
            match tokio::fs::read_to_string(&config_path).await {
                Ok(contents) => {
                    if serde_json::from_str::<serde_json::Value>(&contents).is_ok() {
                        debug!("ConfigWatcher: valid config detected, emitting ConfigReloaded");
                        self.event_bus.publish(DashboardEvent::ConfigReloaded);
                        last_emit = Some(now);
                    } else {
                        warn!(
                            "ConfigWatcher: invalid JSON in {:?}, skipping reload",
                            config_path
                        );
                    }
                }
                Err(e) => {
                    warn!("ConfigWatcher: failed to read {:?}: {}", config_path, e);
                }
            }
        }

        Ok(())
    }
}
