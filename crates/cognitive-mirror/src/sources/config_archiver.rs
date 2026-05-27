//! ConfigArchiverSource — records BrainVersion entries when autotuner promotions occur.
//! Replaces ConfigArchiver.

use std::sync::Arc;

use ai_core::{AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use jiff::Timestamp;

use crate::{AutotunerBridge, BrainVersion, MirrorRepo};

pub struct ConfigArchiverSource {
    repo: MirrorRepo,
    bridge: Option<Arc<dyn AutotunerBridge>>,
}

impl ConfigArchiverSource {
    pub fn new(repo: MirrorRepo, bridge: Option<Arc<dyn AutotunerBridge>>) -> Self {
        Self { repo, bridge }
    }

    async fn get_current_params(&self) -> serde_json::Value {
        if let Some(bridge) = &self.bridge {
            bridge
                .current_champion_params()
                .await
                .unwrap_or(serde_json::json!({}))
        } else {
            serde_json::json!({})
        }
    }

    pub async fn bootstrap(&self, default_params: serde_json::Value) -> common::Result<()> {
        let next = self.repo.get_next_version_number().await?;
        if next > 1 {
            return Ok(());
        }
        let v = BrainVersion {
            version: 1,
            trial_id: None,
            promoted_at: Timestamp::now(),
            params: default_params,
            reason: "Initial brain state".to_string(),
            parent_version: None,
            metrics_at_promotion: serde_json::json!({}),
            reverted: false,
        };
        self.repo.insert_brain_version(&v).await
    }

    async fn record_promotion(
        &self,
        trial_id: Option<String>,
        reason: String,
        metrics: serde_json::Value,
    ) -> common::Result<()> {
        let (params, next) = tokio::join!(
            self.get_current_params(),
            self.repo.get_next_version_number()
        );
        let next = next?;
        let parent = if next > 1 { Some(next - 1) } else { None };
        let v = BrainVersion {
            version: next,
            trial_id,
            promoted_at: Timestamp::now(),
            params,
            reason,
            parent_version: parent,
            metrics_at_promotion: metrics,
            reverted: false,
        };
        self.repo.insert_brain_version(&v).await
    }
}

#[async_trait]
impl MirrorSignalSource for ConfigArchiverSource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "config_archiver",
            subscribed_kinds: &["AutotunerDecision"],
            flush_interval_secs: None,
        }
    }

    fn name(&self) -> &'static str {
        "config-archiver-source"
    }

    async fn accumulate(&self, signal: &AiSignal) -> common::Result<()> {
        if let Some(bus::DomainEvent::AutotunerDecision {
            trial_id,
            verdict,
            improvement_pct,
            ..
        }) = &signal.raw_event
        {
            if verdict == "promoted" {
                let _ = self
                    .record_promotion(
                        Some(trial_id.clone()),
                        format!("Promoted: {:.1}% improvement", improvement_pct),
                        serde_json::json!({"improvement_pct": improvement_pct}),
                    )
                    .await;
            }
        }
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        // Event-driven only.
        Ok(())
    }
}
