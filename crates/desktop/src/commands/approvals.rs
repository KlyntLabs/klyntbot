use std::sync::Arc;

use approval::ApprovalDecision;
use desktop_macros::klynt_command;
use desktop_shared::coding::ApprovalDecisionDto;

use crate::approval::DesktopApprovalChannel;

#[klynt_command]
pub async fn approval_channel_respond(
    approval_id: String,
    decision: ApprovalDecisionDto,
    channel: tauri::State<'_, Arc<DesktopApprovalChannel>>,
) -> bool {
    let internal = map_decision(decision);
    Ok(channel.respond(&approval_id, internal))
}

fn map_decision(dto: ApprovalDecisionDto) -> ApprovalDecision {
    match dto {
        ApprovalDecisionDto::Accept => ApprovalDecision::Once,
        ApprovalDecisionDto::AcceptForSession => ApprovalDecision::Session,
        ApprovalDecisionDto::AcceptWithExecpolicyAmendment { .. } => ApprovalDecision::Forever,
        ApprovalDecisionDto::Decline => ApprovalDecision::Decline {
            reason: "Declined by user".into(),
        },
        ApprovalDecisionDto::Cancel => ApprovalDecision::Cancel,
    }
}
