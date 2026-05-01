//! Direct DomainEventBus subscribers for coding-mirror signal sources.
//!
//! These sources are also registered as `MirrorSignalSource`s via
//! `register_coding_sources`, but that path only sees `AiSignal`s. This module
//! provides a direct `DomainEventBus` subscription for events that are not yet
//! converted to `AiSignal` by the workspace pipeline.

use bus::{DomainEvent, DomainEventBus};
use coding_memory::mirror::coding_signals::{
    ApprovalHistorySignal, RecallCoverageSignal, SkillEffectivenessSignal,
};
use cognitive::mirror::MirrorRepo;
use common::Result;
use std::sync::Arc;

/// Spawn direct bus subscribers for the three coding mirror signal sources.
pub async fn init_coding_subscribers(
    bus: Arc<DomainEventBus>,
    storage_pool: &storage::StoragePool,
) -> Result<()> {
    let mirror_repo = MirrorRepo::new(storage_pool.clone());
    let _approval = Arc::new(ApprovalHistorySignal::new(mirror_repo.clone()));
    let skill_eff = Arc::new(SkillEffectivenessSignal::new(mirror_repo.clone()));
    let recall_cov = Arc::new(RecallCoverageSignal::new(mirror_repo));

    let mut rx = bus.subscribe();
    tokio::spawn(async move {
        while let Ok(evt) = rx.recv().await {
            match evt {
                DomainEvent::SkillRouted { skill_name, .. } => {
                    let _ = skill_eff.observe_skill_activated(&skill_name).await;
                }
                DomainEvent::ToolCallExecuted { tool_name, .. } => {
                    // TODO: wire actual success/failure once ToolCallExecuted carries it.
                    // For now we cannot determine success, so skip attribution.
                    let _ = skill_eff.observe_tool_result(&tool_name, true).await;
                }
                DomainEvent::RetrievalSkillApplied {
                    after_score,
                    skill: _,
                    ..
                } => {
                    let coverage = after_score.clamp(0.0, 1.0);
                    let _ = recall_cov.observe_recall_injected(coverage, false).await;
                }
                // Approval events are not yet on DomainEventBus; when they are,
                // add DomainEvent::ApprovalResolved { .. } => approval.observe_approval_decision(...)
                _ => {}
            }
        }
    });

    Ok(())
}
