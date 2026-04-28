use desktop_macros::klynt_command;
use desktop_shared::commands::{
    AiSuggestionResponse, AnnotationCreateParams, AnnotationResponse, AnnotationUpdateParams,
    LinkedContextParams, LinkedContextResponse,
};
use desktop_shared::CommandResult;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn annotation_create(params: AnnotationCreateParams) -> AnnotationResponse {
    state.annotation_create(params).await
}

#[klynt_command]
pub async fn annotation_update(params: AnnotationUpdateParams) -> AnnotationResponse {
    state.annotation_update(params).await
}

#[klynt_command]
pub async fn annotation_delete(id: String) -> () {
    state.annotation_delete(id).await
}

#[klynt_command]
pub async fn annotation_list_for_note(
    note_id: String,
    limit: Option<i64>,
) -> Vec<AnnotationResponse> {
    state.annotation_list_for_note(note_id, limit).await
}

#[klynt_command]
pub async fn annotation_get_ai_suggestion(
    note_id: String,
    selected_text: String,
) -> AiSuggestionResponse {
    state
        .annotation_get_ai_suggestion(note_id, selected_text)
        .await
}

#[klynt_command]
pub async fn note_get_linked_context(params: LinkedContextParams) -> LinkedContextResponse {
    state.note_get_linked_context(params).await
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
        "annotation_create" => dev::val(
            core.annotation_create(try_field!(dev::parse_params(body)))
                .await,
        ),
        "annotation_update" => dev::val(
            core.annotation_update(try_field!(dev::parse_params(body)))
                .await,
        ),
        "annotation_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.annotation_delete(id).await)
        }
        "annotation_list_for_note" => {
            let note_id = try_field!(dev::get_str(body, "noteId"));
            let limit = body.get("limit").and_then(|v| v.as_i64());
            dev::val(core.annotation_list_for_note(note_id, limit).await)
        }
        "annotation_get_ai_suggestion" => {
            let note_id = try_field!(dev::get_str(body, "noteId"));
            let selected_text = try_field!(dev::get_str(body, "selectedText"));
            dev::val(
                core.annotation_get_ai_suggestion(note_id, selected_text)
                    .await,
            )
        }
        "note_get_linked_context" => dev::val(
            core.note_get_linked_context(try_field!(dev::parse_params(body)))
                .await,
        ),
        _ => return None,
    })
}
