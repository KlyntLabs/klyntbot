//! Per-host approval deduplication cache, modeled on codex-rs/core/src/tools/
//! network_approval.rs::PendingHostApproval.
//!
//! When N parallel calls hit the same `(scheme, host, port)`, only the first
//! invokes Layer1; subsequent callers await the same resolution. Decisions
//! cache for the session (AllowForSession) or evict after one use (AllowOnce).

use dashmap::{mapref::entry::Entry, DashMap};
use std::sync::Arc;
use tokio::sync::watch;
use url::Url;

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct HostKey {
    pub scheme: String,
    pub host: String,
    pub port: u16,
}

impl HostKey {
    pub fn from_url(url: &str) -> common::Result<Self> {
        let u = Url::parse(url).map_err(|e| {
            common::KlyntbotError::Tool(common::ToolError::InvalidParams(format!("bad URL: {e}")))
        })?;
        let host = u.host_str().ok_or_else(|| {
            common::KlyntbotError::Tool(common::ToolError::InvalidParams("URL has no host".into()))
        })?;
        let port = u.port_or_known_default().unwrap_or(0);
        Ok(Self {
            scheme: u.scheme().to_ascii_lowercase(),
            host: host.to_ascii_lowercase(),
            port,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostDecision {
    /// Per-call grant. Cache evicted after resolution.
    AllowOnce,
    /// Session-scoped grant. Cache retained until session ends.
    AllowForSession,
    /// Refused. Cache retained — future calls fail fast.
    Deny,
}

#[derive(Clone)]
enum HostState {
    Pending(watch::Receiver<Option<HostDecision>>),
    Resolved(HostDecision),
}

#[derive(Clone, Default)]
pub struct HostApprovalCache {
    map: Arc<DashMap<HostKey, HostState>>,
}

#[derive(Debug)]
pub enum HostCheckResult {
    /// Decision was already cached.
    Cached(HostDecision),
    /// Another concurrent caller is awaiting first-time resolution.
    AwaitPending(watch::Receiver<Option<HostDecision>>),
    /// First caller for this host. The caller MUST resolve via `cache.resolve(key, decision)`
    /// after evaluating the approval, and ALSO send via `tx` so existing waiters wake up.
    NewlyRegistered { tx: watch::Sender<Option<HostDecision>> },
}

impl HostApprovalCache {
    pub fn check_or_register(&self, key: HostKey) -> HostCheckResult {
        match self.map.entry(key.clone()) {
            Entry::Occupied(slot) => match slot.get().clone() {
                HostState::Pending(rx) => HostCheckResult::AwaitPending(rx),
                HostState::Resolved(d) => HostCheckResult::Cached(d),
            },
            Entry::Vacant(slot) => {
                let (tx, rx) = watch::channel(None);
                slot.insert(HostState::Pending(rx));
                HostCheckResult::NewlyRegistered { tx }
            }
        }
    }

    pub fn resolve(&self, key: HostKey, decision: HostDecision) {
        match decision {
            HostDecision::AllowOnce => {
                // Evict after broadcast — future calls re-enter the approval flow.
                self.map.remove(&key);
            }
            HostDecision::AllowForSession | HostDecision::Deny => {
                self.map
                    .entry(key)
                    .and_modify(|s| *s = HostState::Resolved(decision))
                    .or_insert(HostState::Resolved(decision));
            }
        }
    }
}
