use desktop_shared::commands::{
    PracticeCompleteParams, PracticeCompleteResponse, PracticeConfirmParams,
    PracticeConfirmResponse, PracticeEvalResponse, PracticeGetParams, PracticeListParams,
    PracticeSegmentParams, PracticeSegmentResponse, PracticeSessionResponse, PracticeStartParams,
    PracticeSubmitParams,
};
use desktop_shared::CommandResult;
use desktop_macros::klynt_command;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn practice_segment_note(
    params: PracticeSegmentParams,
) -> PracticeSegmentResponse {
    state.practice_segment_note(params).await
}

#[klynt_command]
pub async fn practice_start_session(
    params: PracticeStartParams,
) -> PracticeSessionResponse {
    state.practice_start_session(params).await
}

#[klynt_command]
pub async fn practice_submit_unit(
    params: PracticeSubmitParams,
) -> PracticeEvalResponse {
    state.practice_submit_unit(params).await
}

#[klynt_command]
pub async fn practice_confirm_unit(
    params: PracticeConfirmParams,
) -> PracticeConfirmResponse {
    state.practice_confirm_unit(params).await
}

#[klynt_command]
pub async fn practice_get_session(
    params: PracticeGetParams,
) -> Option<PracticeSessionResponse> {
    state.practice_get_session(params).await
}

#[klynt_command]
pub async fn practice_complete_session(
    params: PracticeCompleteParams,
) -> PracticeCompleteResponse {
    state.practice_complete_session(params).await
}

#[klynt_command]
pub async fn practice_list_sessions(
    params: PracticeListParams,
) -> Vec<PracticeSessionResponse> {
    state.practice_list_sessions(params).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "practice_segment_note" => dev::val(
            core.practice_segment_note(try_field!(dev::parse_params(body)))
                .await,
        ),
        "practice_start_session" => dev::val(
            core.practice_start_session(try_field!(dev::parse_params(body)))
                .await,
        ),
        "practice_submit_unit" => dev::val(
            core.practice_submit_unit(try_field!(dev::parse_params(body)))
                .await,
        ),
        "practice_confirm_unit" => dev::val(
            core.practice_confirm_unit(try_field!(dev::parse_params(body)))
                .await,
        ),
        "practice_get_session" => dev::val(
            core.practice_get_session(try_field!(dev::parse_params(body)))
                .await,
        ),
        "practice_complete_session" => dev::val(
            core.practice_complete_session(try_field!(dev::parse_params(body)))
                .await,
        ),
        "practice_list_sessions" => dev::val(
            core.practice_list_sessions(try_field!(dev::parse_params(body)))
                .await,
        ),
        _ => return None,
    })
}
