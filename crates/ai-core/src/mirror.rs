//! Mirror pipeline primitives — shared between ai-core consumers and cognitive
//! mirror sources. The runner handles event filtering + flush scheduling so each
//! concrete `MirrorSignalSource` can focus on aggregation + alerting.

use crate::{AiSignal, SignalConsumer};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// Declarative description of one mirror snapshot type.
#[derive(Debug, Clone, Copy)]
pub struct MirrorSnapshotSpec {
    /// Unique snapshot identifier (matches the mirror_<name>_snapshots table).
    pub name: &'static str,
    /// The list of `AiSignal::event_kind` values this source wants to see.
    /// An empty list means "all" (rare; usually a bug).
    pub subscribed_kinds: &'static [&'static str],
    /// If `Some`, the runner drives `flush()` every N seconds. If `None`, the
    /// source is event-driven only (flush is triggered externally or inside
    /// `accumulate`).
    pub flush_interval_secs: Option<u64>,
}

/// One mirror snapshot producer. Accumulates filtered `AiSignal`s and emits a
/// snapshot on flush. Implementations own their own repo handles.
#[async_trait]
pub trait MirrorSignalSource: Send + Sync + 'static {
    const SPEC: MirrorSnapshotSpec;

    /// Human-readable name for logs.
    fn name(&self) -> &'static str;

    /// Handle one matching `AiSignal`. Should not block.
    async fn accumulate(&self, signal: &AiSignal) -> common::Result<()>;

    /// Build + persist the snapshot (if any) and reset the accumulator.
    /// Called on `flush_interval_secs` ticks and once on shutdown.
    async fn flush(&self) -> common::Result<()>;
}

/// Runtime adapter: `MirrorSignalSource` → `SignalConsumer`.
///
/// Filters incoming signals against `SPEC::subscribed_kinds`, forwards matches
/// to `accumulate`, and (optionally) spawns a background task that calls
/// `flush` at `flush_interval_secs` + once on shutdown.
pub struct MirrorSubscriberRunner<S: MirrorSignalSource> {
    source: Arc<S>,
    cancel: CancellationToken,
}

impl<S: MirrorSignalSource> MirrorSubscriberRunner<S> {
    pub fn new(source: Arc<S>, cancel: CancellationToken) -> Arc<Self> {
        Arc::new(Self { source, cancel })
    }

    /// Spawn the background flush loop. Returns the join handle; callers must
    /// keep it alive for the app lifetime. Panics if `SPEC::flush_interval_secs`
    /// is `None` — event-driven sources must not call this.
    pub fn spawn_flush_loop(self: Arc<Self>, override_interval: Duration) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(override_interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            // Skip the immediate first tick — we want the first flush after one full interval.
            interval.tick().await;

            loop {
                tokio::select! {
                    _ = self.cancel.cancelled() => {
                        if let Err(e) = self.source.flush().await {
                            tracing::warn!(source = self.source.name(), error = %e,
                                "MirrorSubscriberRunner: shutdown flush failed");
                        }
                        return;
                    }
                    _ = interval.tick() => {
                        if let Err(e) = self.source.flush().await {
                            tracing::warn!(source = self.source.name(), error = %e,
                                "MirrorSubscriberRunner: interval flush failed");
                        }
                    }
                }
            }
        })
    }

    /// Spawn using the source's declared interval. Returns `None` if the source
    /// is event-driven (no interval). Callers store the handle.
    pub fn spawn_declared_flush_loop(self: Arc<Self>) -> Option<JoinHandle<()>> {
        let secs = S::SPEC.flush_interval_secs?;
        Some(self.spawn_flush_loop(Duration::from_secs(secs)))
    }
}

#[async_trait]
impl<S: MirrorSignalSource> SignalConsumer for MirrorSubscriberRunner<S> {
    fn name(&self) -> &'static str {
        self.source.name()
    }

    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        if !S::SPEC.subscribed_kinds.contains(&signal.event_kind) {
            return Ok(());
        }
        self.source.accumulate(signal).await
    }
}
