//! Approval-history signal source.
//!
//! Tracks recent approval decisions and emits a coding mirror alert when a
//! tool receives 6+ consecutive auto-allow decisions.

use std::collections::VecDeque;

use ai_core::{AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use cognitive::mirror::{snippet_from_alert, MirrorAlert, MirrorAlertSeverity, MirrorRepo};
use tokio::sync::Mutex;
use tracing::warn;

/// Tracks approval-decision history for pattern detection.
pub struct ApprovalHistorySignal {
    repo: MirrorRepo,
    inner: Mutex<Inner>,
}

struct Inner {
    history: VecDeque<(String, String)>,
    pending_alerts: Vec<MirrorAlert>,
}

impl ApprovalHistorySignal {
    /// Construct with a repo handle.
    pub fn new(repo: MirrorRepo) -> Self {
        Self {
            repo,
            inner: Mutex::new(Inner {
                history: VecDeque::with_capacity(50),
                pending_alerts: vec![],
            }),
        }
    }

    /// Record an approval decision.
    pub async fn observe_approval_decision(
        &self,
        tool: &str,
        decision: &str,
        _layer: &str,
    ) -> common::Result<()> {
        let mut g = self.inner.lock().await;
        if g.history.len() == 50 {
            g.history.pop_front();
        }
        g.history
            .push_back((tool.to_string(), decision.to_string()));

        // Detect 6+ consecutive auto-allows for the same tool.
        let recent: Vec<&(String, String)> = g.history.iter().rev().take(6).collect();
        if recent.len() >= 6 && recent.iter().all(|(t, d)| t == tool && d == "allow") {
            g.pending_alerts.push(MirrorAlert::Coding {
                kind: "ApprovalPatternDetected".into(),
                severity: MirrorAlertSeverity::Low,
                payload: serde_json::json!({"consecutive_allows": 6}),
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
impl MirrorSignalSource for ApprovalHistorySignal {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "coding_approval_history",
            subscribed_kinds: &[],
            flush_interval_secs: None,
        }
    }

    fn name(&self) -> &'static str {
        "coding-approval-history-source"
    }

    async fn accumulate(&self, _signal: &AiSignal) -> common::Result<()> {
        // Currently driven via direct DomainEventBus subscription;
        // AiSignal pipeline does not carry approval events.
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        let mut g = self.inner.lock().await;
        let alerts = std::mem::take(&mut g.pending_alerts);
        drop(g);
        for alert in &alerts {
            let snippet = snippet_from_alert(alert);
            if let Err(e) = self.repo.insert_snippet(&snippet).await {
                warn!("ApprovalHistorySignal: failed to insert snippet: {e}");
            }
        }
        Ok(())
    }
}
