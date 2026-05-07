//! Approval response handler — routes user decisions back to the gate's
//! pending-request map so the awaiting tool future resolves.

use approval::{ApprovalDecision, ApprovalGrantsRepo, ApprovalLifetime, GrantRow};
use bus::DomainEvent;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

use crate::desktop_approval_channel::DesktopApprovalChannel;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppApprovalDecision {
    AllowOnce,
    AllowAlways { rule: Option<String> },
    Deny,
    AddRule { starlark_source: String },
}

#[derive(Debug, Error)]
pub enum ApprovalHandlerError {
    #[error("no pending approval found for request_id {0} (likely already resolved or timed out)")]
    NotFound(String),
    #[error("internal channel error: {0}")]
    Channel(String),
    #[error("grants repo error: {0}")]
    Grants(#[from] common::KlyntbotError),
}

#[tracing::instrument(skip(channel, grants_repo, event_bus), fields(request_id = %request_id))]
pub async fn respond_approval(
    channel: Arc<DesktopApprovalChannel>,
    grants_repo: Arc<ApprovalGrantsRepo>,
    event_bus: Option<Arc<bus::DomainEventBus>>,
    request_id: &str,
    decision: AppApprovalDecision,
) -> Result<(), ApprovalHandlerError> {
    let core_decision = match &decision {
        AppApprovalDecision::AllowOnce => ApprovalDecision::Once,
        AppApprovalDecision::AllowAlways { .. } => ApprovalDecision::Forever,
        AppApprovalDecision::Deny => ApprovalDecision::Decline {
            reason: "User denied".into(),
        },
        AppApprovalDecision::AddRule { .. } => ApprovalDecision::Forever,
    };

    let result = channel
        .resolve(request_id, core_decision)
        .map_err(|e| match e {
            crate::desktop_approval_channel::ResolveError::NotFound(id) => {
                ApprovalHandlerError::NotFound(id)
            }
            crate::desktop_approval_channel::ResolveError::SendFailed => {
                ApprovalHandlerError::Channel("oneshot recipient dropped".into())
            }
        });

    // Persist Forever-class decisions after successful resolution so we don't
    // create a grant for a request that timed out or was already resolved.
    if result.is_ok()
        && matches!(
            decision,
            AppApprovalDecision::AllowAlways { .. } | AppApprovalDecision::AddRule { .. }
        )
    {
        if let Some(snapshot) = channel.peek(request_id) {
            let resource_key = approval::extract_path_str_from_args(&snapshot.args);
            let now = jiff::Timestamp::now().as_second();
            let row = GrantRow {
                class: snapshot.class,
                tool_name: snapshot.tool_name,
                action: None,
                resource_key,
                lifetime: ApprovalLifetime::Forever,
                session_id: None,
                granted_at: now,
                expires_at: None,
            };
            let _ = grants_repo.insert(&row).await;
        }
    }

    // Emit ApprovalResolved event after successful resolution.
    if result.is_ok() {
        if let Some(bus) = event_bus {
            let decision_str = if matches!(decision, AppApprovalDecision::Deny) {
                "deny"
            } else {
                "allow"
            };
            bus.publish(DomainEvent::ApprovalResolved {
                request_id: request_id.to_string(),
                decision: decision_str.to_string(),
                decided_by: "user".to_string(),
            });
        }
    }

    result
}
