use desktop_shared::commands::{
    PracticeCompleteParams, PracticeCompleteResponse, PracticeConfirmParams,
    PracticeConfirmResponse, PracticeEvalResponse, PracticeGetParams, PracticeListParams,
    PracticeSegmentParams, PracticeSegmentResponse, PracticeSessionResponse, PracticeStartParams,
    PracticeSubmitParams,
};
use desktop_shared::CommandResult;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
#[specta::specta]
pub async fn practice_segment_note(
    state: State<'_, Arc<AppCore>>,
    params: PracticeSegmentParams,
) -> CommandResult<PracticeSegmentResponse> {
    state.practice_segment_note(params).await
}

#[tauri::command]
#[specta::specta]
pub async fn practice_start_session(
    state: State<'_, Arc<AppCore>>,
    params: PracticeStartParams,
) -> CommandResult<PracticeSessionResponse> {
    state.practice_start_session(params).await
}

#[tauri::command]
#[specta::specta]
pub async fn practice_submit_unit(
    state: State<'_, Arc<AppCore>>,
    params: PracticeSubmitParams,
) -> CommandResult<PracticeEvalResponse> {
    state.practice_submit_unit(params).await
}

#[tauri::command]
#[specta::specta]
pub async fn practice_confirm_unit(
    state: State<'_, Arc<AppCore>>,
    params: PracticeConfirmParams,
) -> CommandResult<PracticeConfirmResponse> {
    state.practice_confirm_unit(params).await
}

#[tauri::command]
#[specta::specta]
pub async fn practice_get_session(
    state: State<'_, Arc<AppCore>>,
    params: PracticeGetParams,
) -> CommandResult<Option<PracticeSessionResponse>> {
    state.practice_get_session(params).await
}

#[tauri::command]
#[specta::specta]
pub async fn practice_complete_session(
    state: State<'_, Arc<AppCore>>,
    params: PracticeCompleteParams,
) -> CommandResult<PracticeCompleteResponse> {
    state.practice_complete_session(params).await
}

#[tauri::command]
#[specta::specta]
pub async fn practice_list_sessions(
    state: State<'_, Arc<AppCore>>,
    params: PracticeListParams,
) -> CommandResult<Vec<PracticeSessionResponse>> {
    state.practice_list_sessions(params).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "practice_segment_note",
    "practice_start_session",
    "practice_submit_unit",
    "practice_confirm_unit",
    "practice_get_session",
    "practice_complete_session",
    "practice_list_sessions",
];

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
