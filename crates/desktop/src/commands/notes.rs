use std::sync::Arc;

use desktop_shared::commands::{
    BacklinkResponse, CreatePersonaParams, FlashcardResponse, HybridSearchResponse,
    InboxCreateParams, InboxItemResponse, InsightEvolutionResponse, InsightQuizSubmitParams,
    InsightReviewResponse, InsightReviewStarted, InsightSaveFlashcardsParams,
    InsightVersionResponse, NoteCreateParams, NoteLinkResponse, NoteResponse,
    NoteSuggestionsResponse, NoteUpdateParams, NoteVersionResponse, NotebookCreateParams,
    NotebookResponse, NotebookUpdateParams, PersonaResponse, RatePersonaParams,
    SetPersonaPinsParams, TabContent, UpdatePersonaParams,
};
use desktop_shared::errors::ApiError;
use tauri::State;

use crate::app_core::AppCore;

// ── Note commands ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn note_list(
    state: State<'_, Arc<AppCore>>,
    notebook_id: Option<String>,
) -> Result<Vec<NoteResponse>, ApiError> {
    state.note_list(notebook_id).await
}

#[tauri::command]
pub async fn note_get(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<NoteResponse, ApiError> {
    state.note_get(id).await
}

#[tauri::command]
pub async fn note_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: NoteCreateParams,
) -> Result<NoteResponse, ApiError> {
    let (result, updates) = state.note_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn note_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: NoteUpdateParams,
) -> Result<NoteResponse, ApiError> {
    let (result, updates) = state.note_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn note_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let (result, updates) = state.note_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn note_search(
    state: State<'_, Arc<AppCore>>,
    query: String,
) -> Result<Vec<NoteResponse>, ApiError> {
    state.note_search(query).await
}

#[tauri::command]
pub async fn note_search_semantic(
    state: State<'_, Arc<AppCore>>,
    query: String,
) -> Result<Vec<NoteResponse>, ApiError> {
    state.note_search_semantic(&query).await
}

#[tauri::command]
pub async fn note_search_hybrid(
    state: State<'_, Arc<AppCore>>,
    query: String,
) -> Result<HybridSearchResponse, ApiError> {
    state.note_search_hybrid(&query).await
}

#[tauri::command]
pub async fn note_links_all(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<NoteLinkResponse>, ApiError> {
    state.note_links_all().await
}

#[tauri::command]
pub async fn note_list_by_entity(
    state: State<'_, Arc<AppCore>>,
    entity_type: String,
    entity_id: String,
) -> Result<Vec<NoteResponse>, ApiError> {
    state.note_list_by_entity(entity_type, entity_id).await
}

// ── Version commands ────────────────────────────────────────────────────

#[tauri::command]
pub async fn note_version_list(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
) -> Result<Vec<NoteVersionResponse>, ApiError> {
    state.note_version_list(note_id).await
}

#[tauri::command]
pub async fn note_version_create(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
) -> Result<NoteVersionResponse, ApiError> {
    state.note_version_create(note_id).await
}

#[tauri::command]
pub async fn note_version_restore(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    version_id: String,
    note_id: String,
) -> Result<NoteResponse, ApiError> {
    let (result, updates) = state.note_version_restore(version_id, note_id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

// ── Attachment commands ─────────────────────────────────────────────────

#[tauri::command]
pub async fn note_save_attachment(
    state: State<'_, Arc<AppCore>>,
    data: String,
    filename: String,
) -> Result<String, ApiError> {
    state.note_save_attachment(data, filename).await
}

// ── Notebook commands ───────────────────────────────────────────────────

#[tauri::command]
pub async fn notebook_list(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<NotebookResponse>, ApiError> {
    state.notebook_list().await
}

#[tauri::command]
pub async fn notebook_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: NotebookCreateParams,
) -> Result<NotebookResponse, ApiError> {
    let (result, updates) = state.notebook_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn notebook_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: NotebookUpdateParams,
) -> Result<NotebookResponse, ApiError> {
    let (result, updates) = state.notebook_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn notebook_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let (result, updates) = state.notebook_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

// ── Archive commands ───────────────────────────────────────────────────

#[tauri::command]
pub async fn note_archive(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), ApiError> {
    let (_, updates) = state.note_archive(&id).await?;
    super::emit_updates(&app, &updates);
    Ok(())
}

#[tauri::command]
pub async fn note_unarchive(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), ApiError> {
    let (_, updates) = state.note_unarchive(&id).await?;
    super::emit_updates(&app, &updates);
    Ok(())
}

#[tauri::command]
pub async fn note_list_archived(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<NoteResponse>, ApiError> {
    state.note_list_archived().await
}

// ── Backlink commands ─────────────────────────────────────────────────

#[tauri::command]
pub async fn note_backlinks(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<Vec<BacklinkResponse>, ApiError> {
    state.note_backlinks(&id).await
}

// ── Suggestion commands ───────────────────────────────────────────────

#[tauri::command]
pub async fn note_suggestions(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<NoteSuggestionsResponse, ApiError> {
    state.note_suggestions(&id).await
}

// ── Tag commands ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn note_tags_all(state: State<'_, Arc<AppCore>>) -> Result<Vec<(String, i64)>, ApiError> {
    state
        .note_repo
        .get_all_tags()
        .await
        .map_err(|e| ApiError::new("STORAGE", e.to_string()))
}

// ── Unlinked mentions ─────────────────────────────────────────────────

#[tauri::command]
pub async fn note_unlinked_mentions(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<Vec<NoteResponse>, ApiError> {
    state.note_unlinked_mentions(&id).await
}

// ── Inbox commands ────────────────────────────────────────────────────

#[tauri::command]
pub async fn inbox_create(
    state: State<'_, Arc<AppCore>>,
    params: InboxCreateParams,
) -> Result<InboxItemResponse, ApiError> {
    state.inbox_create(&params.content).await
}

#[tauri::command]
pub async fn inbox_list(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<InboxItemResponse>, ApiError> {
    state.inbox_list().await
}

#[tauri::command]
pub async fn inbox_delete(state: State<'_, Arc<AppCore>>, id: String) -> Result<(), ApiError> {
    state.inbox_delete(&id).await
}

// ── Insight Review commands ─────────────────────────────────────────

#[tauri::command]
pub async fn note_insight_review(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
    scope_config: Option<desktop_shared::commands::InsightScopeConfigParams>,
) -> Result<InsightReviewStarted, ApiError> {
    state
        .note_insight_review(&note_id, scope_config.as_ref(), None)
        .await
}

#[tauri::command]
pub async fn note_insight_cache_get(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
) -> Result<Option<InsightReviewResponse>, ApiError> {
    state.note_insight_cache_get(&note_id).await
}

#[tauri::command]
pub async fn note_insight_save_flashcards(
    state: State<'_, Arc<AppCore>>,
    params: InsightSaveFlashcardsParams,
) -> Result<Vec<FlashcardResponse>, ApiError> {
    state.insight_save_flashcards(params).await
}

#[tauri::command]
pub async fn note_insight_submit_quiz(
    state: State<'_, Arc<AppCore>>,
    params: InsightQuizSubmitParams,
) -> Result<(), ApiError> {
    state.note_insight_submit_quiz(&params).await
}

#[tauri::command]
pub async fn note_insight_regenerate_tab(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
    tab: String,
) -> Result<TabContent, ApiError> {
    state.note_insight_regenerate_tab(&note_id, &tab).await
}

#[tauri::command]
pub async fn note_insight_list_versions(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
) -> Result<Vec<InsightVersionResponse>, ApiError> {
    state.note_insight_list_versions(&note_id).await
}

#[tauri::command]
pub async fn note_insight_get_evolution(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
) -> Result<InsightEvolutionResponse, ApiError> {
    state.note_insight_get_evolution(&note_id).await
}

#[tauri::command]
pub async fn note_insight_get_version(
    state: State<'_, Arc<AppCore>>,
    insight_id: String,
) -> Result<InsightReviewResponse, ApiError> {
    state.note_insight_get_version(&insight_id).await
}

// ── Persona Management commands ───────────────────────────────────

#[tauri::command]
pub async fn note_insight_list_personas(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<PersonaResponse>, ApiError> {
    state.note_insight_list_personas().await
}

#[tauri::command]
pub async fn note_insight_create_persona(
    state: State<'_, Arc<AppCore>>,
    params: CreatePersonaParams,
) -> Result<PersonaResponse, ApiError> {
    state.note_insight_create_persona(params).await
}

#[tauri::command]
pub async fn note_insight_update_persona(
    state: State<'_, Arc<AppCore>>,
    params: UpdatePersonaParams,
) -> Result<PersonaResponse, ApiError> {
    state.note_insight_update_persona(params).await
}

#[tauri::command]
pub async fn note_insight_delete_persona(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<(), ApiError> {
    state.note_insight_delete_persona(&id).await
}

#[tauri::command]
pub async fn note_insight_toggle_persona(
    state: State<'_, Arc<AppCore>>,
    id: String,
    active: bool,
) -> Result<(), ApiError> {
    state.note_insight_toggle_persona(&id, active).await
}

#[tauri::command]
pub async fn note_insight_set_pins(
    state: State<'_, Arc<AppCore>>,
    params: SetPersonaPinsParams,
) -> Result<(), ApiError> {
    state.note_insight_set_pins(params).await
}

#[tauri::command]
pub async fn note_insight_rate_persona(
    state: State<'_, Arc<AppCore>>,
    params: RatePersonaParams,
) -> Result<(), ApiError> {
    state.note_insight_rate_persona(params).await
}

#[tauri::command]
pub async fn note_insight_auto_generate_persona(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
) -> Result<PersonaResponse, ApiError> {
    state.note_insight_auto_generate_persona(&note_id).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "note_list",
    "note_get",
    "note_create",
    "note_update",
    "note_delete",
    "note_search",
    "note_search_semantic",
    "note_search_hybrid",
    "note_links_all",
    "note_list_by_entity",
    "note_version_list",
    "note_version_create",
    "note_version_restore",
    "note_save_attachment",
    "notebook_list",
    "notebook_create",
    "notebook_update",
    "notebook_delete",
    "note_archive",
    "note_unarchive",
    "note_list_archived",
    "note_backlinks",
    "note_suggestions",
    "note_tags_all",
    "note_unlinked_mentions",
    "inbox_create",
    "inbox_list",
    "inbox_delete",
    "note_insight_review",
    "note_insight_cache_get",
    "note_insight_save_flashcards",
    "note_insight_submit_quiz",
    "note_insight_regenerate_tab",
    "note_insight_list_versions",
    "note_insight_get_evolution",
    "note_insight_get_version",
    "note_insight_list_personas",
    "note_insight_create_persona",
    "note_insight_update_persona",
    "note_insight_delete_persona",
    "note_insight_toggle_persona",
    "note_insight_set_pins",
    "note_insight_rate_persona",
    "note_insight_auto_generate_persona",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "note_list" => dev::val(core.note_list(dev::get(body, "notebook_id")).await),
        "note_get" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.note_get(id).await)
        }
        "note_create" => dev::val_rh(core.note_create(try_field!(dev::parse_params(body))).await),
        "note_update" => dev::val_rh(core.note_update(try_field!(dev::parse_params(body))).await),
        "note_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val_rh(core.note_delete(id).await)
        }
        "note_search" => {
            let query = try_field!(dev::get_str(body, "query"));
            dev::val(core.note_search(query).await)
        }
        "note_search_semantic" => {
            let query = try_field!(dev::get_str(body, "query"));
            dev::val(core.note_search_semantic(&query).await)
        }
        "note_search_hybrid" => {
            let query = try_field!(dev::get_str(body, "query"));
            dev::val(core.note_search_hybrid(&query).await)
        }
        "note_links_all" => dev::val(core.note_links_all().await),
        "note_list_by_entity" => {
            let entity_type = try_field!(dev::get_str(body, "entity_type"));
            let entity_id = try_field!(dev::get_str(body, "entity_id"));
            dev::val(core.note_list_by_entity(entity_type, entity_id).await)
        }
        "note_version_list" => {
            let note_id = try_field!(dev::get_str(body, "note_id"));
            dev::val(core.note_version_list(note_id).await)
        }
        "note_version_create" => {
            let note_id = try_field!(dev::get_str(body, "note_id"));
            dev::val(core.note_version_create(note_id).await)
        }
        "note_version_restore" => {
            let version_id = try_field!(dev::get_str(body, "version_id"));
            let note_id = try_field!(dev::get_str(body, "note_id"));
            dev::val_rh(core.note_version_restore(version_id, note_id).await)
        }
        "note_save_attachment" => {
            let data = try_field!(dev::get_str(body, "data"));
            let filename = try_field!(dev::get_str(body, "filename"));
            dev::val(core.note_save_attachment(data, filename).await)
        }
        "notebook_list" => dev::val(core.notebook_list().await),
        "notebook_create" => dev::val_rh(
            core.notebook_create(try_field!(dev::parse_params(body)))
                .await,
        ),
        "notebook_update" => dev::val_rh(
            core.notebook_update(try_field!(dev::parse_params(body)))
                .await,
        ),
        "notebook_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val_rh(core.notebook_delete(id).await)
        }
        "note_archive" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val_rh(core.note_archive(&id).await)
        }
        "note_unarchive" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val_rh(core.note_unarchive(&id).await)
        }
        "note_list_archived" => dev::val(core.note_list_archived().await),
        "note_backlinks" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.note_backlinks(&id).await)
        }
        "note_suggestions" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.note_suggestions(&id).await)
        }
        "note_tags_all" => dev::val(
            core.note_repo
                .get_all_tags()
                .await
                .map_err(|e| ApiError::new("STORAGE", e.to_string())),
        ),
        "note_unlinked_mentions" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.note_unlinked_mentions(&id).await)
        }
        "inbox_create" => dev::val(
            core.inbox_create(
                &try_field!(dev::parse_params::<
                    desktop_shared::commands::InboxCreateParams,
                >(body))
                .content,
            )
            .await,
        ),
        "inbox_list" => dev::val(core.inbox_list().await),
        "inbox_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.inbox_delete(&id).await)
        }
        // note_insight_review is handled inline in dev_server/dispatch.rs
        // (needs SSE emitter injection, like chat_send)
        "note_insight_cache_get" => {
            let id = try_field!(dev::get_str(body, "noteId"));
            dev::val(core.note_insight_cache_get(&id).await)
        }
        "note_insight_save_flashcards" => dev::val(
            core.insight_save_flashcards(try_field!(dev::parse_params::<
                desktop_shared::commands::InsightSaveFlashcardsParams,
            >(body)))
                .await,
        ),
        "note_insight_submit_quiz" => dev::val(
            core.note_insight_submit_quiz(&try_field!(dev::parse_params::<
                desktop_shared::commands::InsightQuizSubmitParams,
            >(body)))
                .await,
        ),
        "note_insight_regenerate_tab" => {
            let id = try_field!(dev::get_str(body, "noteId"));
            let tab = try_field!(dev::get_str(body, "tab"));
            dev::val(core.note_insight_regenerate_tab(&id, &tab).await)
        }
        "note_insight_list_versions" => {
            let id = try_field!(dev::get_str(body, "noteId"));
            dev::val(core.note_insight_list_versions(&id).await)
        }
        "note_insight_get_evolution" => {
            let id = try_field!(dev::get_str(body, "noteId"));
            dev::val(core.note_insight_get_evolution(&id).await)
        }
        "note_insight_get_version" => {
            let id = try_field!(dev::get_str(body, "insightId"));
            dev::val(core.note_insight_get_version(&id).await)
        }
        "note_insight_list_personas" => dev::val(core.note_insight_list_personas().await),
        "note_insight_create_persona" => dev::val(
            core.note_insight_create_persona(try_field!(dev::parse_params(body)))
                .await,
        ),
        "note_insight_update_persona" => dev::val(
            core.note_insight_update_persona(try_field!(dev::parse_params(body)))
                .await,
        ),
        "note_insight_delete_persona" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.note_insight_delete_persona(&id).await)
        }
        "note_insight_toggle_persona" => {
            let id = try_field!(dev::get_str(body, "id"));
            let active = body.get("active").and_then(|v| v.as_bool()).unwrap_or(true);
            dev::val(core.note_insight_toggle_persona(&id, active).await)
        }
        "note_insight_set_pins" => dev::val(
            core.note_insight_set_pins(try_field!(dev::parse_params(body)))
                .await,
        ),
        "note_insight_rate_persona" => dev::val(
            core.note_insight_rate_persona(try_field!(dev::parse_params(body)))
                .await,
        ),
        "note_insight_auto_generate_persona" => {
            let note_id = try_field!(dev::get_str(body, "noteId"));
            dev::val(core.note_insight_auto_generate_persona(&note_id).await)
        }
        _ => return None,
    })
}
