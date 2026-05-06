//! Skill-effectiveness signal source.
//!
//! Tracks `SkillActivated` → subsequent tool-call success rate. Emits a coding
//! mirror alert when a skill precedes >3 failed tool calls without success.

use std::collections::HashMap;

use ai_core::{AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use cognitive::mirror::{snippet_from_alert, MirrorAlert, MirrorAlertSeverity, MirrorRepo};
use tokio::sync::Mutex;
use tracing::warn;

/// Tracks skill effectiveness via tool-call outcomes.
pub struct SkillEffectivenessSignal {
    repo: MirrorRepo,
    inner: Mutex<Inner>,
}

struct Inner {
    active: HashMap<String, EffectivenessState>,
    pending_alerts: Vec<MirrorAlert>,
}

#[derive(Default)]
struct EffectivenessState {
    consecutive_failures: u32,
    has_succeeded: bool,
}

impl SkillEffectivenessSignal {
    /// Construct with a repo handle.
    pub fn new(repo: MirrorRepo) -> Self {
        Self {
            repo,
            inner: Mutex::new(Inner {
                active: HashMap::new(),
                pending_alerts: vec![],
            }),
        }
    }

    /// Record a skill activation.
    pub async fn observe_skill_activated(&self, skill_id: &str) -> common::Result<()> {
        let mut g = self.inner.lock().await;
        g.active.entry(skill_id.to_string()).or_default();
        Ok(())
    }

    /// Record a tool result attributed to currently-active skills.
    pub async fn observe_tool_result(&self, skill_id: &str, success: bool) -> common::Result<()> {
        let mut g = self.inner.lock().await;
        let (should_alert, failures, tool_id) = {
            let st = g.active.get_mut(skill_id).map(|s| {
                if success {
                    s.has_succeeded = true;
                    s.consecutive_failures = 0;
                } else {
                    s.consecutive_failures += 1;
                }
                s.consecutive_failures
            });
            match st {
                Some(failures) => {
                    let alert = failures >= 3;
                    (alert, failures, skill_id.to_string())
                }
                None => (false, 0, skill_id.to_string()),
            }
        };
        // Prune entries that have succeeded to prevent unbounded growth.
        g.active.retain(|_, s| !s.has_succeeded);
        if should_alert {
            g.pending_alerts.push(MirrorAlert::Coding {
                kind: "SkillUnderperforming".into(),
                severity: MirrorAlertSeverity::Medium,
                payload: serde_json::json!({
                    "skill_id": tool_id,
                    "consecutive_failures": failures,
                }),
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
impl MirrorSignalSource for SkillEffectivenessSignal {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "coding_skill_effectiveness",
            subscribed_kinds: &[],
            flush_interval_secs: None,
        }
    }

    fn name(&self) -> &'static str {
        "coding-skill-effectiveness-source"
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
                warn!("SkillEffectivenessSignal: failed to insert snippet: {e}");
            }
        }
        Ok(())
    }
}
