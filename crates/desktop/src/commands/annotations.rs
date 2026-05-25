use desktop_macros::klynt_command;
use desktop_shared::commands::{
    AiSuggestionResponse, AnnotationCreateParams, AnnotationResponse, AnnotationUpdateParams,
    LinkedContextParams, LinkedContextResponse,
};

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
