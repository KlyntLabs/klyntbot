//! Watches config.json for external changes and propagates hot-reloadable
//! fields to the live agent pipeline.

use std::sync::Arc;

use config::HotConfig;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

/// Spawns a background task that polls config.json for changes every 30 seconds.
///
/// When a change is detected in hot-reloadable fields, updates the shared
/// `HotConfig` and the `AppCore.config` RwLock.
///
/// Returns a `CancellationToken` to stop the watcher.
pub fn start_config_watcher(
    app_config: Arc<RwLock<config::Config>>,
    hot_config: Arc<RwLock<HotConfig>>,
    shutdown_token: CancellationToken,
) -> CancellationToken {
    let watcher_token = shutdown_token.child_token();
    let token = watcher_token.clone();

    tokio::spawn(async move {
        info!("config watcher started (30s poll interval)");
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        interval.tick().await; // Skip immediate first tick (config was just loaded at init)
        let mut last_mtime: Option<std::time::SystemTime> = None;

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    debug!("config watcher shutting down");
                    break;
                }
                _ = interval.tick() => {
                    let previous = hot_config.read().await;
                    if let Some((new_config, diff, new_hot)) = config::reload_if_changed(&*previous, &mut last_mtime).await {
                        info!(
                            model_changed = diff.model_changed,
                            temp_changed = diff.temperature_changed,
                            tokens_changed = diff.max_tokens_changed,
                            iterations_changed = diff.max_tool_iterations_changed,
                            timeout_changed = diff.safety_timeout_changed,
                            budget_changed = diff.budget_changed,
                            "config hot-reload: applying changes"
                        );

                        // Update shared HotConfig (read by agent pipeline)
                        {
                            let mut guard = hot_config.write().await;
                            *guard = new_hot;
                        }

                        // Update full config in AppCore (read by settings handlers)
                        {
                            let mut guard = app_config.write().await;
                            *guard = new_config;
                        }
                    }
                }
            }
        }
    });

    watcher_token
}
