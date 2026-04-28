//! Running apps source — pure signal producer.
//!
//! Refresh polls `NSRunningApplication`, updates the shared `RunningSignals`
//! map keyed by bundle ID, and removes stale entries. Search returns empty:
//! AppIndex consumes the signal map and emits the unified `Application` row
//! for each running app.

use crate::search::signals::{RunningSignal, RunningSignals};
use crate::types::*;
use platform_macos::apps::RunningApp;
use smol_str::SmolStr;
use std::collections::HashSet;

#[derive(Clone)]
pub struct RunningAppsSource {
    signals: RunningSignals,
}

impl RunningAppsSource {
    pub fn new(signals: RunningSignals) -> Self {
        Self { signals }
    }
}

#[async_trait::async_trait]
impl super::SearchSource for RunningAppsSource {
    fn name(&self) -> &'static str {
        "running_apps"
    }

    async fn refresh(&self) {
        let snapshot = tokio::task::spawn_blocking(|| {
            platform_macos::apps::running_applications()
        })
        .await
        .unwrap_or_default();

        tracing::debug!("Running apps snapshot: {} entries", snapshot.len());
        apply_snapshot(&self.signals, &snapshot);
    }

    async fn search(&self, _query: &str, _limit: usize) -> Vec<LauncherItem> {
        Vec::new()
    }
}

/// Replace the contents of `signals` with the given snapshot, dropping any
/// stale entries no longer present and skipping snapshot rows without a bundle ID.
pub(crate) fn apply_snapshot(signals: &RunningSignals, snapshot: &[RunningApp]) {
    let live: HashSet<SmolStr> = snapshot
        .iter()
        .filter_map(|a| a.bundle_id.as_deref().map(SmolStr::new))
        .collect();

    signals.retain(|k, _| live.contains(k));

    for app in snapshot {
        if let Some(bid) = app.bundle_id.as_deref() {
            signals.insert(
                SmolStr::new(bid),
                RunningSignal {
                    pid: app.pid as u32,
                    path: app.path.clone().unwrap_or_default(),
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::signals::new_running_signals;
    use crate::search::SearchSource;

    #[tokio::test]
    async fn search_returns_empty() {
        let signals = new_running_signals();
        let src = RunningAppsSource::new(signals);
        let results = src.search("anything", 10).await;
        assert!(results.is_empty(), "RunningAppsSource is signal-only");
    }

    #[test]
    fn refresh_replaces_signal_snapshot() {
        // Drives the synchronous refresh helper, not the async refresh wrapper,
        // because the macOS NSWorkspace call cannot be mocked here. The helper
        // takes the snapshot as input and updates the signal map in-place.
        use platform_macos::apps::RunningApp;

        let signals = new_running_signals();
        // Pre-populate with a stale entry that the next snapshot omits.
        signals.insert(
            smol_str::SmolStr::new("com.stale.App"),
            RunningSignal { pid: 1, path: std::path::PathBuf::new() },
        );

        let snapshot = vec![
            RunningApp {
                name: "Safari".into(),
                bundle_id: Some("com.apple.Safari".into()),
                pid: 99,
                path: Some("/Applications/Safari.app".into()),
            },
            RunningApp {
                name: "NoBundle".into(),
                bundle_id: None,    // must be ignored
                pid: 100,
                path: None,
            },
        ];

        apply_snapshot(&signals, &snapshot);

        assert_eq!(signals.len(), 1, "stale entry dropped, no-bundle entry skipped");
        let safari = signals.get(&smol_str::SmolStr::new("com.apple.Safari")).unwrap();
        assert_eq!(safari.pid, 99);
    }
}
