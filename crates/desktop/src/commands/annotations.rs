use desktop_shared::commands::{
    AiSuggestionResponse, AnnotationCreateParams, AnnotationResponse, AnnotationUpdateParams,
    LinkedContextParams, LinkedContextResponse,
};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn annotation_create(
    state: State<'_, Arc<AppCore>>,
    params: AnnotationCreateParams,
) -> Result<AnnotationResponse, ApiError> {
    state.annotation_create(params).await
}

#[tauri::command]
pub async fn annotation_update(
    state: State<'_, Arc<AppCore>>,
    params: AnnotationUpdateParams,
) -> Result<AnnotationResponse, ApiError> {
    state.annotation_update(params).await
}

#[tauri::command]
pub async fn annotation_delete(state: State<'_, Arc<AppCore>>, id: String) -> Result<(), ApiError> {
    state.annotation_delete(id).await
}

#[tauri::command]
pub async fn annotation_list_for_note(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
    limit: Option<i64>,
) -> Result<Vec<AnnotationResponse>, ApiError> {
    state.annotation_list_for_note(note_id, limit).await
}

#[tauri::command]
pub async fn annotation_get_ai_suggestion(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
    selected_text: String,
) -> Result<AiSuggestionResponse, ApiError> {
    state
        .annotation_get_ai_suggestion(note_id, selected_text)
        .await
}

#[tauri::command]
pub async fn note_get_linked_context(
    state: State<'_, Arc<AppCore>>,
    params: LinkedContextParams,
) -> Result<LinkedContextResponse, ApiError> {
    state.note_get_linked_context(params).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "annotation_create",
    "annotation_update",
    "annotation_delete",
    "annotation_list_for_note",
    "annotation_get_ai_suggestion",
    "note_get_linked_context",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
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
