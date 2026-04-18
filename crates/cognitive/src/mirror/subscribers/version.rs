//! ConfigArchiver — records BrainVersion entries when autotuner promotions occur.

use std::sync::Arc;

use bus::DomainEvent;
use jiff::Timestamp;
use common::Result;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use super::super::repo::MirrorRepo;
use super::super::types::{AutotunerBridge, BrainVersion};

pub struct ConfigArchiver {
    repo: MirrorRepo,
    bridge: Option<Arc<dyn AutotunerBridge>>,
}

impl ConfigArchiver {
    pub fn new(repo: MirrorRepo, bridge: Option<Arc<dyn AutotunerBridge>>) -> Self {
        Self { repo, bridge }
    }

    #[cfg(test)]
    pub fn new_for_test(repo: MirrorRepo) -> Self {
        Self { repo, bridge: None }
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

    pub async fn bootstrap(&self, default_params: serde_json::Value) -> Result<()> {
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

    pub async fn record_promotion(
        &self,
        trial_id: Option<String>,
        reason: String,
        metrics: serde_json::Value,
    ) -> Result<()> {
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

    pub async fn run(self, mut rx: broadcast::Receiver<DomainEvent>, shutdown: CancellationToken) {
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => break,
                event = rx.recv() => {
                    match event {
                        Ok(DomainEvent::AutotunerDecision { trial_id, verdict, improvement_pct, .. }) => {
                            if verdict == "promoted" {
                                let _ = self
                                    .record_promotion(
                                        Some(trial_id),
                                        format!("Promoted: {:.1}% improvement", improvement_pct),
                                        serde_json::json!({"improvement_pct": improvement_pct}),
                                    )
                                    .await;
                            }
                        }
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("ConfigArchiver lagged {n} events");
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bootstrap_creates_version_1() {
        let repo = crate::mirror::test_mirror_repo().await;
        let archiver = ConfigArchiver::new_for_test(repo.clone());

        archiver
            .bootstrap(serde_json::json!({"temperature": 0.7}))
            .await
            .unwrap();

        let v1 = repo.get_brain_version(1).await.unwrap();
        assert!(v1.is_some());
        let got = v1.unwrap();
        assert_eq!(got.version, 1);
        assert_eq!(got.reason, "Initial brain state");
        assert!(got.trial_id.is_none());
        assert!(got.parent_version.is_none());
    }

    #[tokio::test]
    async fn test_bootstrap_skips_if_exists() {
        let repo = crate::mirror::test_mirror_repo().await;
        let archiver = ConfigArchiver::new_for_test(repo.clone());

        archiver
            .bootstrap(serde_json::json!({"temperature": 0.7}))
            .await
            .unwrap();
        // Bootstrap again — should be a no-op
        archiver
            .bootstrap(serde_json::json!({"temperature": 0.9}))
            .await
            .unwrap();

        let versions = repo.get_brain_versions().await.unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].version, 1);
    }

    #[tokio::test]
    async fn test_record_promotion() {
        let repo = crate::mirror::test_mirror_repo().await;
        let archiver = ConfigArchiver::new_for_test(repo.clone());

        archiver
            .bootstrap(serde_json::json!({"temperature": 0.7}))
            .await
            .unwrap();

        archiver
            .record_promotion(
                Some("trial-abc".to_string()),
                "Promoted: 5.0% improvement".to_string(),
                serde_json::json!({"improvement_pct": 5.0}),
            )
            .await
            .unwrap();

        let versions = repo.get_brain_versions().await.unwrap();
        assert_eq!(versions.len(), 2);

        // get_brain_versions returns DESC order, so index 0 is v2
        let v2 = &versions[0];
        assert_eq!(v2.version, 2);
        assert_eq!(v2.parent_version, Some(1));
        assert_eq!(v2.trial_id, Some("trial-abc".to_string()));
    }
}
