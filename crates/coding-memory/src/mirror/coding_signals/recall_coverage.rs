//! Recall-coverage signal source.
//!
//! Tracks `RecallInjected.coverage_score` and emits a coding mirror alert when
//! 3+ consecutive turns fall below 0.3 coverage.

use std::collections::VecDeque;

use ai_core::{AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use cognitive::mirror::{snippet_from_alert, MirrorAlert, MirrorAlertSeverity, MirrorRepo};
use tokio::sync::Mutex;
use tracing::warn;

/// Tracks recall-coverage trends.
pub struct RecallCoverageSignal {
    repo: MirrorRepo,
    inner: Mutex<Inner>,
}

struct Inner {
    recent: VecDeque<f32>,
    pending_alerts: Vec<MirrorAlert>,
}

impl RecallCoverageSignal {
    /// Construct with a repo handle.
    pub fn new(repo: MirrorRepo) -> Self {
        Self {
            repo,
            inner: Mutex::new(Inner {
                recent: VecDeque::with_capacity(5),
                pending_alerts: vec![],
            }),
        }
    }

    /// Record a recall-injected coverage score.
    pub async fn observe_recall_injected(
        &self,
        coverage: f32,
        _dead_end: bool,
    ) -> common::Result<()> {
        let mut g = self.inner.lock().await;
        if g.recent.len() == 5 {
            g.recent.pop_front();
        }
        g.recent.push_back(coverage);
        if g.recent.len() >= 3 && g.recent.iter().rev().take(3).all(|c| *c < 0.3) {
            let recent_vec: Vec<f32> = g.recent.iter().rev().take(3).copied().collect();
            g.pending_alerts.push(MirrorAlert::Coding {
                kind: "RecallCoverageLow".into(),
                severity: MirrorAlertSeverity::Low,
                payload: serde_json::json!({"recent_coverage": recent_vec}),
            });
        }
        Ok(())
    }

    /// Drain pending alerts (test helper).
    pub async fn drain(&self) -> common::Result<Vec<MirrorAlert>> {
        let mut g = self.inner.lock().await;
        Ok(std::mem::take(&mut g.pending_alerts))
    }
}

#[async_trait]
impl MirrorSignalSource for RecallCoverageSignal {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "coding_recall_coverage",
            subscribed_kinds: &[],
            flush_interval_secs: None,
        }
    }

    fn name(&self) -> &'static str {
        "coding-recall-coverage-source"
    }

    async fn accumulate(&self, _signal: &AiSignal) -> common::Result<()> {
        // Currently driven via direct DomainEventBus subscription.
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        let mut g = self.inner.lock().await;
        let alerts = std::mem::take(&mut g.pending_alerts);
        drop(g);
        for alert in &alerts {
            let snippet = snippet_from_alert(alert);
            if let Err(e) = self.repo.insert_snippet(&snippet).await {
                warn!("RecallCoverageSignal: failed to insert snippet: {e}");
            }
        }
        Ok(())
    }
}
