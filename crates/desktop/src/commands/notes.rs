use std::sync::Arc;

use desktop_shared::commands::{
    BacklinkResponse, ChangesSummaryResponse, CreatePersonaParams, DeckPreferenceResponse,
    DeckSummaryResponse, FlashcardCreateParams, FlashcardDistractorParams,
    FlashcardDistractorResponse, FlashcardExplainParams, FlashcardExplainResponse,
    FlashcardGenerateParams, FlashcardGenerateResponse, FlashcardListParams, FlashcardResponse,
    FlashcardReviewParams, FlashcardSaveGeneratedParams, FlashcardSubmitAnswerParams,
    FlashcardUpdateParams, GradeResultResponse, HybridSearchResponse, InboxCreateParams,
    InboxItemResponse, InsightEvolutionResponse, InsightQuizSubmitParams, InsightReviewResponse,
    InsightReviewStarted, InsightSaveFlashcardsParams, InsightVersionResponse,
    KnowledgeGrowthResponse, NoteCreateParams, NoteEditingFinishedParams, NoteLinkResponse,
    NoteResponse, NoteRetentionHealthResponse, NoteSuggestionsResponse, NoteUpdateParams,
    NoteVersionResponse, NotebookCreateParams, NotebookResponse, NotebookUpdateParams,
    PersonaChatParams, PersonaChatResponse, PersonaResponse, RatePersonaParams,
    RecentLearningSession, ScenarioChallengeResponse, ScopePreviewParams, ScopePreviewResponse,
    SetPersonaPinsParams, StrugglingCardResponse, TabContent, UpdatePersonaParams,
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
    squad_id: Option<String>,
) -> Result<InsightReviewStarted, ApiError> {
    state
        .note_insight_review(&note_id, scope_config.as_ref(), squad_id.as_deref(), None)
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
pub async fn note_insight_debate(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
    squad_id: Option<String>,
) -> Result<(), ApiError> {
    state
        .note_insight_debate(&note_id, squad_id.as_deref())
        .await
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

#[tauri::command]
pub async fn note_insight_generate_scenario(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
) -> Result<ScenarioChallengeResponse, ApiError> {
    state.note_insight_generate_scenario(&note_id).await
}

#[tauri::command]
pub async fn note_insight_changes_summary(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
) -> Result<Option<ChangesSummaryResponse>, ApiError> {
    state.note_insight_changes_summary(&note_id).await
}

#[tauri::command]
pub async fn note_insight_knowledge_growth(
    state: State<'_, Arc<AppCore>>,
    days: Option<u32>,
) -> Result<KnowledgeGrowthResponse, ApiError> {
    state.note_insight_knowledge_growth(days.unwrap_or(7)).await
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

#[tauri::command]
pub async fn note_insight_persona_chat(
    state: State<'_, Arc<AppCore>>,
    params: PersonaChatParams,
) -> Result<PersonaChatResponse, ApiError> {
    state.note_insight_persona_chat(&params).await
}

#[tauri::command]
pub async fn note_insight_preview_scope(
    state: State<'_, Arc<AppCore>>,
    params: ScopePreviewParams,
) -> Result<ScopePreviewResponse, ApiError> {
    state.note_insight_preview_scope(params).await
}

// ── Flashcard Review commands ───────────────────────────────────

#[tauri::command]
pub async fn flashcard_list_decks(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<DeckSummaryResponse>, ApiError> {
    state.flashcard_list_decks().await
}

#[tauri::command]
pub async fn flashcard_get_due(
    state: State<'_, Arc<AppCore>>,
    deck: String,
    limit: Option<i64>,
) -> Result<Vec<FlashcardResponse>, ApiError> {
    state.flashcard_get_due(&deck, limit.unwrap_or(10)).await
}

#[tauri::command]
pub async fn flashcard_record_review(
    state: State<'_, Arc<AppCore>>,
    params: FlashcardReviewParams,
) -> Result<FlashcardResponse, ApiError> {
    state.flashcard_record_review(params).await
}

#[tauri::command]
pub async fn flashcard_get(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<FlashcardResponse, ApiError> {
    state.flashcard_get(&id).await
}

#[tauri::command]
pub async fn flashcard_create(
    state: State<'_, Arc<AppCore>>,
    params: FlashcardCreateParams,
) -> Result<FlashcardResponse, ApiError> {
    state.flashcard_create(params).await
}

#[tauri::command]
pub async fn flashcard_update(
    state: State<'_, Arc<AppCore>>,
    params: FlashcardUpdateParams,
) -> Result<FlashcardResponse, ApiError> {
    state.flashcard_update(params).await
}

#[tauri::command]
pub async fn flashcard_list_cards(
    state: State<'_, Arc<AppCore>>,
    params: FlashcardListParams,
) -> Result<Vec<FlashcardResponse>, ApiError> {
    state.flashcard_list_cards(params).await
}

#[tauri::command]
pub async fn flashcard_delete(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<bool, ApiError> {
    state.flashcard_delete(&id).await
}

#[tauri::command]
pub async fn flashcard_get_all_due(
    state: State<'_, Arc<AppCore>>,
    limit: i64,
) -> Result<Vec<FlashcardResponse>, ApiError> {
    state.flashcard_get_all_due(limit).await
}

#[tauri::command]
pub async fn flashcard_total_due(state: State<'_, Arc<AppCore>>) -> Result<i64, ApiError> {
    state.flashcard_total_due().await
}

#[tauri::command]
pub async fn flashcard_list_struggling(
    state: State<'_, Arc<AppCore>>,
    limit: Option<i64>,
) -> Result<Vec<StrugglingCardResponse>, ApiError> {
    state.flashcard_list_struggling(limit.unwrap_or(5)).await
}

#[tauri::command]
pub async fn flashcard_generate(
    state: State<'_, Arc<AppCore>>,
    params: FlashcardGenerateParams,
) -> Result<FlashcardGenerateResponse, ApiError> {
    state.flashcard_generate(params).await
}

#[tauri::command]
pub async fn flashcard_save_generated(
    state: State<'_, Arc<AppCore>>,
    params: FlashcardSaveGeneratedParams,
) -> Result<Vec<FlashcardResponse>, ApiError> {
    state.flashcard_save_generated(params).await
}

// ── Active Recall commands ──────────────────────────────────────────

#[tauri::command]
pub async fn flashcard_submit_answer(
    state: State<'_, Arc<AppCore>>,
    params: FlashcardSubmitAnswerParams,
) -> Result<GradeResultResponse, ApiError> {
    state.flashcard_submit_answer(params).await
}

#[tauri::command]
pub async fn flashcard_explain_answer(
    state: State<'_, Arc<AppCore>>,
    params: FlashcardExplainParams,
) -> Result<FlashcardExplainResponse, ApiError> {
    state.flashcard_explain_answer(params).await
}

#[tauri::command]
pub async fn flashcard_generate_distractors(
    state: State<'_, Arc<AppCore>>,
    params: FlashcardDistractorParams,
) -> Result<FlashcardDistractorResponse, ApiError> {
    state.flashcard_generate_distractors(params).await
}

#[tauri::command]
pub async fn flashcard_save_mode_preference(
    state: State<'_, Arc<AppCore>>,
    deck: String,
    mode: String,
) -> Result<(), ApiError> {
    state
        .deck_preference_repo()?
        .set(&deck, &mode)
        .await
        .map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn flashcard_get_mode_preference(
    state: State<'_, Arc<AppCore>>,
    deck: String,
) -> Result<Option<DeckPreferenceResponse>, ApiError> {
    let row = state
        .deck_preference_repo()?
        .get(&deck)
        .await
        .map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?;
    Ok(row.map(|r| DeckPreferenceResponse {
        deck: r.deck,
        answer_mode: r.answer_mode,
    }))
}

#[tauri::command]
pub async fn flashcard_get_prerequisites(
    state: State<'_, Arc<AppCore>>,
    card_id: String,
) -> Result<Vec<FlashcardResponse>, ApiError> {
    state.flashcard_get_prerequisites(&card_id).await
}

#[tauri::command]
pub async fn flashcard_save_session(
    state: State<'_, Arc<AppCore>>,
    params: desktop_shared::commands::ReviewSessionSaveParams,
) -> Result<(), ApiError> {
    state.flashcard_save_session(params).await
}

#[tauri::command]
pub async fn flashcard_recent_learning_sessions(
    state: State<'_, Arc<AppCore>>,
    limit: Option<usize>,
) -> Result<Vec<RecentLearningSession>, ApiError> {
    state
        .flashcard_recent_learning_sessions(limit.unwrap_or(3))
        .await
}

// ── Retention Health ────────────────────────────────────────────────

#[tauri::command]
pub async fn note_retention_health(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
) -> Result<Option<NoteRetentionHealthResponse>, ApiError> {
    state.note_retention_health(note_id).await
}

// ── Editing finished command ─────────────────────────────────────────

#[tauri::command]
pub async fn note_editing_finished(
    state: State<'_, Arc<AppCore>>,
    params: NoteEditingFinishedParams,
) -> Result<(), ApiError> {
    state.note_editing_finished(params).await
}

// ── Import / Export ──────────────────────────────────────────────

#[tauri::command]
pub async fn note_import_files(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: desktop_shared::commands::NoteImportParams,
) -> Result<desktop_shared::commands::NoteImportResult, ApiError> {
    let result = state.note_import_files(params).await?;
    super::emit_updates(
        &app,
        &[::app_core::EntityUpdate {
            kind: desktop_shared::types::EntityKind::Note,
            id: "import".into(),
        }],
    );
    Ok(result)
}

#[tauri::command]
pub async fn note_export(
    state: State<'_, Arc<AppCore>>,
    _app: tauri::AppHandle,
    params: desktop_shared::commands::NoteExportParams,
) -> Result<desktop_shared::commands::NoteExportResult, ApiError> {
    state.note_export(params).await
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
    "note_insight_debate",
    "note_insight_list_versions",
    "note_insight_get_evolution",
    "note_insight_get_version",
    "note_insight_generate_scenario",
    "note_insight_changes_summary",
    "note_insight_knowledge_growth",
    "note_insight_list_personas",
    "note_insight_create_persona",
    "note_insight_update_persona",
    "note_insight_delete_persona",
    "note_insight_toggle_persona",
    "note_insight_set_pins",
    "note_insight_rate_persona",
    "note_insight_auto_generate_persona",
    "note_insight_persona_chat",
    "note_insight_preview_scope",
    "flashcard_list_decks",
    "flashcard_get_due",
    "flashcard_record_review",
    "flashcard_get",
    "flashcard_create",
    "flashcard_update",
    "flashcard_list_cards",
    "flashcard_delete",
    "flashcard_get_all_due",
    "flashcard_total_due",
    "flashcard_list_struggling",
    "flashcard_generate",
    "flashcard_save_generated",
    "flashcard_submit_answer",
    "flashcard_explain_answer",
    "flashcard_generate_distractors",
    "flashcard_save_mode_preference",
    "flashcard_get_mode_preference",
    "flashcard_get_prerequisites",
    "flashcard_save_session",
    "flashcard_recent_learning_sessions",
    "note_retention_health",
    "note_editing_finished",
    "note_import_files",
    "note_export",
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
        "note_insight_debate" => {
            let id = try_field!(dev::get_str(body, "noteId"));
            let squad_id = body
                .get("squadId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            dev::val(core.note_insight_debate(&id, squad_id.as_deref()).await)
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
        "note_insight_generate_scenario" => {
            let id = try_field!(dev::get_str(body, "noteId"));
            dev::val(core.note_insight_generate_scenario(&id).await)
        }
        "note_insight_changes_summary" => {
            let id = try_field!(dev::get_str(body, "noteId"));
            dev::val(core.note_insight_changes_summary(&id).await)
        }
        "note_insight_knowledge_growth" => {
            let days: Option<u32> = dev::get(body, "days");
            dev::val(core.note_insight_knowledge_growth(days.unwrap_or(7)).await)
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
        "note_insight_persona_chat" => {
            let params: PersonaChatParams = try_field!(dev::parse_params(body));
            dev::val(core.note_insight_persona_chat(&params).await)
        }
        "note_insight_preview_scope" => dev::val(
            core.note_insight_preview_scope(try_field!(dev::parse_params::<
                desktop_shared::commands::ScopePreviewParams,
            >(body)))
                .await,
        ),
        "flashcard_list_decks" => dev::val(core.flashcard_list_decks().await),
        "flashcard_get_due" => {
            let deck: String = try_field!(dev::get_str(body, "deck"));
            let limit: Option<i64> = dev::get(body, "limit");
            dev::val(core.flashcard_get_due(&deck, limit.unwrap_or(10)).await)
        }
        "flashcard_record_review" => {
            let params: FlashcardReviewParams = try_field!(dev::parse_params(body));
            dev::val(core.flashcard_record_review(params).await)
        }
        "flashcard_get" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.flashcard_get(&id).await)
        }
        "flashcard_create" => dev::val(
            core.flashcard_create(try_field!(dev::parse_params(body)))
                .await,
        ),
        "flashcard_update" => dev::val(
            core.flashcard_update(try_field!(dev::parse_params(body)))
                .await,
        ),
        "flashcard_list_cards" => dev::val(
            core.flashcard_list_cards(try_field!(dev::parse_params(body)))
                .await,
        ),
        "flashcard_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.flashcard_delete(&id).await)
        }
        "flashcard_get_all_due" => {
            let limit = body.get("limit").and_then(|v| v.as_i64()).unwrap_or(50);
            dev::val(core.flashcard_get_all_due(limit).await)
        }
        "flashcard_total_due" => dev::val(core.flashcard_total_due().await),
        "flashcard_list_struggling" => {
            let limit = body.get("limit").and_then(|v| v.as_i64()).unwrap_or(5);
            dev::val(core.flashcard_list_struggling(limit).await)
        }
        "flashcard_generate" => dev::val(
            core.flashcard_generate(try_field!(dev::parse_params(body)))
                .await,
        ),
        "flashcard_save_generated" => dev::val(
            core.flashcard_save_generated(try_field!(dev::parse_params(body)))
                .await,
        ),
        "flashcard_submit_answer" => dev::val(
            core.flashcard_submit_answer(try_field!(dev::parse_params(body)))
                .await,
        ),
        "flashcard_explain_answer" => dev::val(
            core.flashcard_explain_answer(try_field!(dev::parse_params(body)))
                .await,
        ),
        "flashcard_generate_distractors" => dev::val(
            core.flashcard_generate_distractors(try_field!(dev::parse_params(body)))
                .await,
        ),
        "flashcard_save_mode_preference" => {
            let deck = try_field!(dev::get_str(body, "deck"));
            let mode = try_field!(dev::get_str(body, "mode"));
            let repo = match core.deck_preference_repo() {
                Ok(r) => r,
                Err(e) => return Some(Err(e)),
            };
            dev::val(
                repo.set(&deck, &mode)
                    .await
                    .map(|_| ())
                    .map_err(|e| ApiError::new("DB_ERROR", e.to_string())),
            )
        }
        "flashcard_get_mode_preference" => {
            let deck = try_field!(dev::get_str(body, "deck"));
            let repo = match core.deck_preference_repo() {
                Ok(r) => r,
                Err(e) => return Some(Err(e)),
            };
            dev::val(
                repo.get(&deck)
                    .await
                    .map(|row| {
                        row.map(|r| DeckPreferenceResponse {
                            deck: r.deck,
                            answer_mode: r.answer_mode,
                        })
                    })
                    .map_err(|e| ApiError::new("DB_ERROR", e.to_string())),
            )
        }
        "flashcard_get_prerequisites" => {
            let card_id = try_field!(dev::get_str(body, "cardId"));
            dev::val(core.flashcard_get_prerequisites(&card_id).await)
        }
        "flashcard_save_session" => dev::val(
            core.flashcard_save_session(try_field!(dev::parse_params::<
                desktop_shared::commands::ReviewSessionSaveParams,
            >(body)))
                .await,
        ),
        "flashcard_recent_learning_sessions" => {
            let limit = body
                .get("limit")
                .and_then(|v| v.as_u64())
                .map(|l| l as usize);
            dev::val(
                core.flashcard_recent_learning_sessions(limit.unwrap_or(3))
                    .await,
            )
        }
        "note_retention_health" => {
            let note_id = try_field!(dev::get_str(body, "noteId"));
            dev::val(core.note_retention_health(note_id).await)
        }
        "note_editing_finished" => dev::val(
            core.note_editing_finished(try_field!(dev::parse_params(body)))
                .await,
        ),
        "note_import_files" => Err(ApiError::new(
            "UNSUPPORTED",
            "Import requires the desktop app",
        )),
        "note_export" => Err(ApiError::new(
            "UNSUPPORTED",
            "Export requires the desktop app",
        )),
        _ => return None,
    })
}
