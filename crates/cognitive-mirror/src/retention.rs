//! Periodic cleanup of aged mirror rows.

use crate::MirrorRepo;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy)]
pub struct MirrorRetentionConfig {
    pub routing_snapshot_days: u32,
    pub snippet_days: u32,
    pub narrative_days: u32,
    pub disabled_meta_rule_days: u32,
    pub reverted_brain_version_days: u32,
    pub trial_preview_days: u32,
    pub task_focus_snapshot_days: u32,
    pub sweep_interval_secs: u64,
}

impl Default for MirrorRetentionConfig {
    fn default() -> Self {
        Self {
            routing_snapshot_days: 90,
            snippet_days: 30,
            narrative_days: 365,
            disabled_meta_rule_days: 180,
            reverted_brain_version_days: 730,
            trial_preview_days: 180,
            task_focus_snapshot_days: 90,
            sweep_interval_secs: 24 * 3600, // daily
        }
    }
}

pub struct MirrorRetentionService;

impl MirrorRetentionService {
    /// Spawn the daily sweep. Returns the join handle; caller keeps it alive.
    pub fn spawn(
        repo: Arc<MirrorRepo>,
        config: MirrorRetentionConfig,
        cancel: CancellationToken,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(Duration::from_secs(config.sweep_interval_secs));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            interval.tick().await; // Skip immediate first tick.
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => return,
                    _ = interval.tick() => {
                        Self::sweep_once(&repo, &config).await;
                    }
                }
            }
        })
    }

    pub async fn sweep_once(repo: &MirrorRepo, config: &MirrorRetentionConfig) {
        let (r1, r2, r3, r4, r5, r6, r7) = tokio::join!(
            repo.cleanup_old_snapshots(config.routing_snapshot_days),
            repo.cleanup_old_snippets(config.snippet_days),
            repo.cleanup_old_trend_narratives(config.narrative_days),
            repo.cleanup_old_meta_rules(config.disabled_meta_rule_days),
            repo.cleanup_reverted_brain_versions(config.reverted_brain_version_days),
            repo.cleanup_old_trial_previews(config.trial_preview_days),
            repo.cleanup_old_task_focus_snapshots(config.task_focus_snapshot_days),
        );
        for (name, res) in [
            ("snapshots", r1),
            ("snippets", r2),
            ("narratives", r3),
            ("meta_rules", r4),
            ("brain_versions", r5),
            ("trial_previews", r6),
            ("task_focus_snapshots", r7),
        ] {
            if let Err(e) = res {
                tracing::warn!("mirror retention cleanup_old_{name} failed: {e}");
            }
        }
        tracing::debug!("mirror retention sweep completed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sweep_once_does_not_panic_on_empty_tables() {
        let repo = crate::test_mirror_repo().await;
        MirrorRetentionService::sweep_once(&repo, &MirrorRetentionConfig::default()).await;
    }
}
