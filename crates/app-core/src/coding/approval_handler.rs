use klynt_core::approval::{
    decision::{ApprovalDecision, ApprovalLayer},
    PendingApprovalsMap,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

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
    #[error("no pending approval for request_id={0}")]
    NotFound(String),
}

#[tracing::instrument(skip(pending), err)]
pub async fn respond_approval(
    pending: &Arc<PendingApprovalsMap>,
    request_id: &str,
    decision: AppApprovalDecision,
) -> Result<(), ApprovalHandlerError> {
    let mapped = match decision {
        AppApprovalDecision::AllowOnce => ApprovalDecision::Auto {
            allowed: true,
            layer: ApprovalLayer::Layer1Declarative,
            reason: "user: allow once".into(),
            rule_matched: None,
        },
        AppApprovalDecision::AllowAlways { rule } => ApprovalDecision::Auto {
            allowed: true,
            layer: ApprovalLayer::Layer1Declarative,
            reason: format!(
                "user: allow always{}",
                rule.as_deref()
                    .map(|r| format!(" ({r})"))
                    .unwrap_or_default()
            ),
            rule_matched: rule,
        },
        AppApprovalDecision::Deny => ApprovalDecision::Auto {
            allowed: false,
            layer: ApprovalLayer::Layer1Declarative,
            reason: "user: deny".into(),
            rule_matched: None,
        },
        AppApprovalDecision::AddRule { .. } => ApprovalDecision::Auto {
            allowed: true,
            layer: ApprovalLayer::Layer2Starlark,
            reason: "user: added rule (Plan 4 will persist)".into(),
            rule_matched: None,
        },
    };
    if !pending.contains(request_id) {
        return Err(ApprovalHandlerError::NotFound(request_id.into()));
    }
    pending.resolve(request_id, mapped);
    // "Allow always" persistence to config.json + Starlark rule writing
    // happens here in Plan 4; for now we just resolve.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn responds_resolves_pending() {
        let map = Arc::new(PendingApprovalsMap::new());
        let (tx, rx) = tokio::sync::oneshot::channel();
        map.register("req-x".into(), tx);
        respond_approval(&map, "req-x", AppApprovalDecision::AllowOnce)
            .await
            .unwrap();
        let got = rx.await.unwrap();
        assert!(got.allowed());
    }

    #[tokio::test]
    async fn unknown_request_id_returns_not_found() {
        let map = Arc::new(PendingApprovalsMap::new());
        let r = respond_approval(&map, "nope", AppApprovalDecision::Deny).await;
        assert!(matches!(r, Err(ApprovalHandlerError::NotFound(_))));
    }
}
