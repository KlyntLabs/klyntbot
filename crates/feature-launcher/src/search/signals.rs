//! Bundle-ID-keyed signal maps shared between launcher sources.
//!
//! `AppIndex` consumes these maps at query time to decorate the unified
//! `Application` row with live "is running" + cumulative attention data.
//! `RunningAppsSource` writes into `RunningSignals` on each refresh.
//! `AttentionSource` writes into `AttentionSignals` from inside `search`.

use dashmap::DashMap;
use smol_str::SmolStr;
use std::path::PathBuf;
use std::sync::Arc;

/// Live "is running" snapshot for a single app.
#[derive(Clone, Debug)]
pub struct RunningSignal {
    pub pid: u32,
    pub path: PathBuf,
}

/// Cumulative time-tracking stats for a single app.
#[derive(Clone, Debug)]
pub struct AttentionStat {
    pub attention_secs: i64,
    pub category: Option<SmolStr>,
    pub last_used_at: jiff::Timestamp,
}

/// Bundle ID → live "is running" snapshot. Refreshed by RunningAppsSource.
pub type RunningSignals = Arc<DashMap<SmolStr, RunningSignal>>;

/// Bundle ID → cumulative time-tracking stats. Updated by AttentionSource.
pub type AttentionSignals = Arc<DashMap<SmolStr, AttentionStat>>;

/// Helper: construct an empty `RunningSignals` map.
pub fn new_running_signals() -> RunningSignals {
    Arc::new(DashMap::new())
}

/// Helper: construct an empty `AttentionSignals` map.
pub fn new_attention_signals() -> AttentionSignals {
    Arc::new(DashMap::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_signals_round_trip() {
        let signals = new_running_signals();
        let bid = SmolStr::new("com.apple.Safari");
        signals.insert(
            bid.clone(),
            RunningSignal {
                pid: 99,
                path: PathBuf::from("/Applications/Safari.app"),
            },
        );
        let got = signals.get(&bid).expect("expected Safari signal");
        assert_eq!(got.pid, 99);
    }

    #[test]
    fn attention_stat_serializes_timestamp() {
        let stat = AttentionStat {
            attention_secs: 3600,
            category: Some(SmolStr::new("browsing")),
            last_used_at: "2026-04-28T12:00:00Z".parse().unwrap(),
        };
        assert_eq!(stat.attention_secs, 3600);
        assert_eq!(stat.category.as_deref(), Some("browsing"));
    }

    #[test]
    fn running_signals_retain_drops_missing_keys() {
        let signals = new_running_signals();
        signals.insert(
            SmolStr::new("com.apple.Safari"),
            RunningSignal { pid: 1, path: PathBuf::new() },
        );
        signals.insert(
            SmolStr::new("com.apple.Mail"),
            RunningSignal { pid: 2, path: PathBuf::new() },
        );

        let live: std::collections::HashSet<SmolStr> =
            std::iter::once(SmolStr::new("com.apple.Safari")).collect();
        signals.retain(|k, _| live.contains(k));

        assert_eq!(signals.len(), 1);
        assert!(signals.contains_key(&SmolStr::new("com.apple.Safari")));
        assert!(!signals.contains_key(&SmolStr::new("com.apple.Mail")));
    }
}
