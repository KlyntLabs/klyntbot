use desktop_macros::klynt_command;
use desktop_shared::coding::ApprovalDecisionDto;
use desktop_shared::errors::ApiError;

use crate::commands::dev_helpers as dev;

#[klynt_command]
pub async fn approval_respond(approval_id: String, decision: ApprovalDecisionDto) -> () {
    let internal = map_decision(decision);
    app_core::coding::approval_handler::respond_approval(
        &state.pending_approvals,
        &approval_id,
        internal,
    )
    .await
    .map_err(|e| ApiError::new("APPROVAL_ERROR", e.to_string()))
}

fn map_decision(
    dto: ApprovalDecisionDto,
) -> app_core::coding::approval_handler::AppApprovalDecision {
    match dto {
        ApprovalDecisionDto::Accept => {
            app_core::coding::approval_handler::AppApprovalDecision::AllowOnce
        }
        ApprovalDecisionDto::AcceptForSession => {
            app_core::coding::approval_handler::AppApprovalDecision::AllowAlways { rule: None }
        }
        ApprovalDecisionDto::AcceptWithExecpolicyAmendment {
            execpolicy_amendment,
        } => app_core::coding::approval_handler::AppApprovalDecision::AddRule {
            starlark_source: execpolicy_amendment.starlark_source.unwrap_or_default(),
        },
        ApprovalDecisionDto::Decline | ApprovalDecisionDto::Cancel => {
            app_core::coding::approval_handler::AppApprovalDecision::Deny
        }
    }
}

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &::app_core::state::AppCore,
    body: &serde_json::Value,
) -> Option<desktop_shared::CommandResult<serde_json::Value>> {
    Some(match cmd {
        "approval_respond" => {
            let approval_id = dev::get_str(body, "approvalId").unwrap_or_default();
            let decision: ApprovalDecisionDto =
                match serde_json::from_value(body.get("decision").cloned().unwrap_or_default()) {
                    Ok(d) => d,
                    Err(e) => {
                        return Some(Err(ApiError::new("INVALID_PARAMS", e.to_string())))
                    }
                };
            let internal = map_decision(decision);
            match app_core::coding::approval_handler::respond_approval(
                &core.pending_approvals,
                &approval_id,
                internal,
            )
            .await
            {
                Ok(()) => Ok(serde_json::json!({})),
                Err(e) => Err(ApiError::new("APPROVAL_ERROR", e.to_string())),
            }
        }
        _ => return None,
    })
}
