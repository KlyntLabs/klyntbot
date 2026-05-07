use std::sync::Arc;

use desktop_macros::klynt_command;
use desktop_shared::coding::ApprovalDecisionDto;
use tauri::Manager;

use app_core::coding::approval_handler::AppApprovalDecision;

#[klynt_command]
pub async fn approval_channel_respond(
    app: tauri::AppHandle,
    approval_id: String,
    decision: ApprovalDecisionDto,
) -> () {
    let core = app.state::<Arc<app_core::AppCore>>();
    let app_decision = map_decision(decision);
    core.respond_approval(&approval_id, app_decision)
        .await
        .map_err(|e| desktop_shared::errors::ApiError::new("APPROVAL_ERROR", e.to_string()))
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
        ApprovalDecisionDto::Decline => AppApprovalDecision::Deny,
        ApprovalDecisionDto::Cancel => AppApprovalDecision::Deny,
    }
}
