use serde::{Deserialize, Serialize};
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
    #[error("approval system not available")]
    NotAvailable,
}

pub async fn respond_approval(
    _request_id: &str,
    _decision: AppApprovalDecision,
) -> Result<(), ApprovalHandlerError> {
    Err(ApprovalHandlerError::NotAvailable)
}
