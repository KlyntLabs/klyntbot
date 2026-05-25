use desktop_macros::klynt_command;
use desktop_shared::commands::{
    PracticeCompleteParams, PracticeCompleteResponse, PracticeConfirmParams,
    PracticeConfirmResponse, PracticeEvalResponse, PracticeGetParams, PracticeListParams,
    PracticeSegmentParams, PracticeSegmentResponse, PracticeSessionResponse, PracticeStartParams,
    PracticeSubmitParams,
};
#[klynt_command]
pub async fn practice_segment_note(params: PracticeSegmentParams) -> PracticeSegmentResponse {
    state.practice_segment_note(params).await
}

#[klynt_command]
pub async fn practice_start_session(params: PracticeStartParams) -> PracticeSessionResponse {
    state.practice_start_session(params).await
}

#[klynt_command]
pub async fn practice_submit_unit(params: PracticeSubmitParams) -> PracticeEvalResponse {
    state.practice_submit_unit(params).await
}

#[klynt_command]
pub async fn practice_confirm_unit(params: PracticeConfirmParams) -> PracticeConfirmResponse {
    state.practice_confirm_unit(params).await
}

#[klynt_command]
pub async fn practice_get_session(params: PracticeGetParams) -> Option<PracticeSessionResponse> {
    state.practice_get_session(params).await
}

#[klynt_command]
pub async fn practice_complete_session(params: PracticeCompleteParams) -> PracticeCompleteResponse {
    state.practice_complete_session(params).await
}

#[klynt_command]
pub async fn practice_list_sessions(params: PracticeListParams) -> Vec<PracticeSessionResponse> {
    state.practice_list_sessions(params).await
}
