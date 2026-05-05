use ai_core::mirror::{MirrorSignalSource, MirrorSnapshotSpec};
use dashmap::DashMap;
use std::sync::Arc;
use storage::repos::{CodingApprovalHistoryRepo, HistoryEntry};
use tools_core::events::ToolEvent;

pub struct ApprovalHistorySource {
    repo: Arc<CodingApprovalHistoryRepo>,
    pending: DashMap<String, PendingReq>,
}

struct PendingReq {
    tool: String,
    args_hash: String,
    layer: String,
    repo_id: String,
}

impl ApprovalHistorySource {
    pub fn new(repo: Arc<CodingApprovalHistoryRepo>) -> Self {
        Self {
            repo,
            pending: DashMap::new(),
        }
    }

    /// Record a pending approval request.
    pub fn observe_request(
        &self,
        request_id: &str,
        tool: &str,
        args_hash: &str,
        layer: &str,
        repo_id: &str,
    ) {
        self.pending.insert(
            request_id.to_string(),
            PendingReq {
                tool: tool.to_string(),
                args_hash: args_hash.to_string(),
                layer: layer.to_string(),
                repo_id: repo_id.to_string(),
            },
        );
    }

    /// Record an approval resolution (writes to SQL if a matching request is pending).
    pub async fn observe_resolution(&self, request_id: &str, decision: &str, decided_by: &str) {
        if let Some((_, pending)) = self.pending.remove(request_id) {
            let _ = self
                .repo
                .record(HistoryEntry {
                    tool: pending.tool,
                    args_hash: pending.args_hash,
                    repo_id: pending.repo_id,
                    decision: decision.to_string(),
                    decided_by: decided_by.to_string(),
                    layer: pending.layer,
                })
                .await;
        }
    }

    /// Backward-compatible observer that accepts `ToolEvent` (used in tests).
    pub async fn observe(&self, ev: &ToolEvent, repo_id: &str) {
        match ev {
            ToolEvent::ApprovalRequested {
                request_id,
                tool,
                args_hash,
                layer,
                ..
            } => {
                self.observe_request(request_id, tool, args_hash, layer, repo_id);
            }
            ToolEvent::ApprovalResolved {
                request_id,
                decision,
                decided_by,
                ..
            } => {
                self.observe_resolution(request_id, decision, decided_by)
                    .await;
            }
            _ => {}
        }
    }
}

#[async_trait::async_trait]
impl MirrorSignalSource for ApprovalHistorySource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "approval_history",
            subscribed_kinds: &[],
            flush_interval_secs: None,
        }
    }

    fn name(&self) -> &'static str {
        "approval_history"
    }

    async fn accumulate(&self, _signal: &ai_core::AiSignal) -> common::Result<()> {
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        Ok(())
    }
}
