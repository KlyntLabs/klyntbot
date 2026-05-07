use ai_core::mirror::{MirrorSignalSource, MirrorSnapshotSpec};
use dashmap::DashMap;
use std::sync::Arc;
use storage::repos::{CodingApprovalHistoryRepo, HistoryEntry};
use tools_core::events::ToolEvent;

const MIN_APPROVAL_COUNT: u32 = 3;
const MIN_APPROVAL_RATE: f32 = 0.80;
const RECENCY_WINDOW_DAYS: i64 = 30;

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

/// A suggested grant pattern derived from approval history.
#[derive(Debug, Clone)]
pub struct SuggestedPattern {
    /// Human-readable label, e.g. "Allow bash with args-hash abc123".
    pub label: String,
    /// The args_hash that matched.
    pub args_hash: String,
    /// Number of prior approvals matching this pattern.
    pub approval_count: u32,
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

    /// Suggest a grant pattern for a tool call based on approval history.
    /// Returns `Some(SuggestedPattern)` if the tool has >= 80% approval rate
    /// across >= 3 prior approvals for the same args_hash within the recency window.
    pub async fn suggest_pattern(&self, tool: &str, args_hash: &str) -> Option<SuggestedPattern> {
        let stats = self.repo.tool_pattern_stats(tool, Some(args_hash), RECENCY_WINDOW_DAYS).await.ok()?;
        for (hash, approvals, total) in &stats {
            if hash != args_hash {
                continue;
            }
            if *total < MIN_APPROVAL_COUNT {
                continue;
            }
            let rate = *approvals as f32 / *total as f32;
            if rate < MIN_APPROVAL_RATE {
                continue;
            }
            return Some(SuggestedPattern {
                label: format!("Allow {} ({} prior approvals)", tool, approvals),
                args_hash: hash.clone(),
                approval_count: *approvals,
            });
        }
        None
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
