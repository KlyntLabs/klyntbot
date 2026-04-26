//! Coding routing drift source.
//!
//! Tracks `SkillRouted` events that originate from project-specific skills and
//! emits alerts when a project's skill usage drops significantly between
//! consecutive flush periods.

use std::collections::HashMap;

use ai_core::{mirror::mirror_flush_secs, AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use tokio::sync::Mutex;
use tracing::{info, warn};

use cognitive::mirror::{snippet_from_alert, MirrorAlert, MirrorAlertSeverity, MirrorRepo};

/// Source that detects project-skill obsolescence by comparing current-period
/// routing counts against the previous period.
pub struct CodingRoutingSource {
    repo: MirrorRepo,
    /// Counts for the current (not-yet-flushed) period.
    current: Mutex<HashMap<String, u32>>,
    /// Counts from the previous flushed period.
    history: Mutex<HashMap<String, u32>>,
}

impl CodingRoutingSource {
    /// Construct.
    pub fn new(repo: MirrorRepo) -> Self {
        Self {
            repo,
            current: Mutex::new(HashMap::new()),
            history: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl MirrorSignalSource for CodingRoutingSource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "coding_routing",
            subscribed_kinds: &["SkillRouted"],
            flush_interval_secs: Some(mirror_flush_secs(3600)),
        }
    }

    fn name(&self) -> &'static str {
        "coding-routing-source"
    }

    async fn accumulate(&self, signal: &AiSignal) -> common::Result<()> {
        if let Some(bus::DomainEvent::SkillRouted {
            skill_name,
            source,
            ..
        }) = &signal.raw_event
        {
            if source.starts_with("project:") {
                let mut current = self.current.lock().await;
                *current.entry(skill_name.clone()).or_insert(0) += 1;
            }
        }
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        let (alerts, new_history) = {
            let mut current = self.current.lock().await;
            let history = self.history.lock().await;

            let mut alert_skills = Vec::new();
            for (skill, hist_count) in history.iter() {
                if *hist_count < 5 {
                    continue;
                }
                let curr_count = current.get(skill).copied().unwrap_or(0);
                if curr_count <= (*hist_count as f64 * 0.5).floor() as u32 {
                    alert_skills.push(skill.clone());
                }
            }

            let new_history = std::mem::take(&mut *current);
            (alert_skills, new_history)
        };

        for skill in &alerts {
            let alert = MirrorAlert::Coding {
                kind: "project_skill_obsolete".into(),
                severity: MirrorAlertSeverity::Medium,
                payload: serde_json::json!({
                    "skill": skill,
                }),
            };
            let snippet = snippet_from_alert(&alert);
            if let Err(e) = self.repo.insert_snippet(&snippet).await {
                warn!("CodingRoutingSource: failed to insert snippet: {e}");
            } else {
                info!(
                    skill = skill,
                    "CodingRoutingSource: project skill obsolete alert emitted"
                );
            }
        }

        let mut history = self.history.lock().await;
        *history = new_history;

        Ok(())
    }
}
