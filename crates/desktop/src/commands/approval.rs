use std::sync::Arc;

use desktop_macros::klynt_command;
use desktop_shared::coding::ApprovalDecisionDto;
use desktop_shared::errors::ApiError;
use tauri::Manager;

use app_core::coding::approval_handler::AppApprovalDecision;

#[klynt_command]
pub async fn approval_respond(
    app: tauri::AppHandle,
    approval_id: String,
    decision: ApprovalDecisionDto,
) -> () {
    let core = app.state::<Arc<app_core::AppCore>>();
    let internal = map_decision(decision);
    core.respond_approval(&approval_id, internal)
        .await
        .map_err(|e| ApiError::new("APPROVAL_ERROR", e.to_string()))
}

fn map_decision(dto: ApprovalDecisionDto) -> AppApprovalDecision {
    match dto {
        ApprovalDecisionDto::Accept => AppApprovalDecision::AllowOnce,
        ApprovalDecisionDto::AcceptForSession => AppApprovalDecision::AllowAlways { rule: None },
        ApprovalDecisionDto::AcceptWithExecpolicyAmendment {
            execpolicy_amendment,
        } => AppApprovalDecision::AddRule {
            starlark_source: execpolicy_amendment.starlark_source.unwrap_or_default(),
        },
        ApprovalDecisionDto::Decline | ApprovalDecisionDto::Cancel => AppApprovalDecision::Deny,
    }
}

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &::app_core::state::AppCore,
    body: &serde_json::Value,
) -> Option<desktop_shared::CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "approval_respond" => {
            let approval_id = try_field!(dev::get_str(body, "approvalId"));
            let decision: ApprovalDecisionDto = try_field!(dev::require(body, "decision"));
            let internal = map_decision(decision);
            match core.respond_approval(&approval_id, internal).await {
                Ok(()) => Ok(serde_json::json!({})),
                Err(e) => Err(ApiError::new("APPROVAL_ERROR", e.to_string())),
            }
        }
        _ => return None,
    })
}
