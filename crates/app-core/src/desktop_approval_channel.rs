//! Desktop-channel impl of ApprovalChannel.
//!
//! Owns a pending-request map keyed by request_id and a oneshot::Sender per
//! pending request. The gate's `channel.request().await` future blocks on the
//! oneshot until the user clicks Approve/Deny in the UI, at which point
//! `respond_approval` calls `resolve(...)` to wake the future.

use approval::{
    ApprovalCapabilities, ApprovalChannel, ApprovalClass, ApprovalDecision, ApprovalRequest,
};
use dashmap::DashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use uuid::Uuid;

const APPROVAL_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("no pending approval found for request_id {0}")]
    NotFound(String),
    #[error("oneshot send failed (recipient dropped)")]
    SendFailed,
}

#[derive(Debug, Clone)]
pub struct PendingSnapshot {
    pub tool_name: String,
    pub args: serde_json::Value,
    pub cwd: PathBuf,
    pub class: ApprovalClass,
}

struct PendingEntry {
    sender: oneshot::Sender<ApprovalDecision>,
    tool_name: String,
    args: serde_json::Value,
    cwd: PathBuf,
    class: ApprovalClass,
}

pub struct DesktopApprovalChannel {
    pending: Arc<DashMap<String, PendingEntry>>,
    emitter: Arc<dyn crate::events::AppEventEmitter>,
}

impl DesktopApprovalChannel {
    pub fn new(emitter: Arc<dyn crate::events::AppEventEmitter>) -> Self {
        Self {
            pending: Arc::new(DashMap::new()),
            emitter,
        }
    }

    /// Wake the awaiting `request()` future for `request_id` with `decision`.
    /// Returns NotFound if no pending entry exists (timed out, cancelled, or
    /// already resolved).
    pub fn resolve(&self, request_id: &str, decision: ApprovalDecision) -> Result<(), ResolveError> {
        let (_id, entry) = self
            .pending
            .remove(request_id)
            .ok_or_else(|| ResolveError::NotFound(request_id.to_string()))?;
        entry.sender.send(decision).map_err(|_| ResolveError::SendFailed)
    }

    /// Read-only snapshot of a pending entry (used for emitting ApprovalResolved events).
    pub fn peek(&self, request_id: &str) -> Option<PendingSnapshot> {
        self.pending.get(request_id).map(|e| PendingSnapshot {
            tool_name: e.tool_name.clone(),
            args: e.args.clone(),
            cwd: e.cwd.clone(),
            class: e.class,
        })
    }

    /// Build a `GrantRow` from a pending snapshot for Forever-class decisions.
    pub fn build_grant_row(&self, snapshot: &PendingSnapshot) -> approval::GrantRow {
        let now = jiff::Timestamp::now().as_second();
        approval::GrantRow {
            class: snapshot.class,
            tool_name: snapshot.tool_name.clone(),
            action: None,
            resource_key: approval::extract_path_str_from_args(&snapshot.args),
            lifetime: approval::ApprovalLifetime::Forever,
            session_id: None,
            granted_at: now,
            expires_at: None,
        }
    }

    pub fn pending_ids(&self) -> Vec<String> {
        self.pending.iter().map(|e| e.key().clone()).collect()
    }
}

#[async_trait::async_trait]
impl ApprovalChannel for DesktopApprovalChannel {
    async fn request(&self, req: ApprovalRequest) -> ApprovalDecision {
        let request_id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel::<ApprovalDecision>();

        self.pending.insert(
            request_id.clone(),
            PendingEntry {
                sender: tx,
                tool_name: req.tool_name.clone(),
                args: req.args.clone(),
                cwd: req.ctx.cwd.clone(),
                class: req.class,
            },
        );

        // Emit approval:request event via the generic emitter
        let payload = serde_json::json!({
            "id": request_id,
            "tool_name": req.tool_name,
            "action": req.action,
            "class": req.class,
            "preview": req.preview,
            "suggested_grant": req.suggested_grant,
        });
        self.emitter.emit_event("agent:approval_requested", payload);

        match tokio::time::timeout(APPROVAL_TIMEOUT, rx).await {
            Ok(Ok(decision)) => decision,
            Ok(Err(_)) => {
                self.pending.remove(&request_id);
                ApprovalDecision::Decline {
                    reason: "internal: oneshot dropped".into(),
                }
            }
            Err(_) => {
                self.pending.remove(&request_id);
                ApprovalDecision::Decline {
                    reason: "Approval timed out (600s)".into(),
                }
            }
        }
    }

    fn capabilities(&self) -> ApprovalCapabilities {
        ApprovalCapabilities {
            supports_inline: true,
            supports_classes: HashSet::from([
                ApprovalClass::Safe,
                ApprovalClass::Sensitive,
                ApprovalClass::Destructive,
                ApprovalClass::Admin,
            ]),
        }
    }
}
