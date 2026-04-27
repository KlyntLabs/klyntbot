use std::sync::Arc;

use desktop_macros::{klynt_command, klynt_raw_command};
use desktop_shared::commands::{
    BacklinkResponse, ChangesSummaryResponse, CreatePersonaParams, DeckPreferenceResponse,
    DeckSummaryResponse, FlashcardCreateParams, FlashcardDistractorParams,
    FlashcardDistractorResponse, FlashcardExplainParams, FlashcardExplainResponse,
    FlashcardGenerateParams, FlashcardGenerateResponse, FlashcardListParams, FlashcardResponse,
    FlashcardReviewParams, FlashcardSaveGeneratedParams, FlashcardSubmitAnswerParams,
    FlashcardUpdateParams, GradeResultResponse, HybridSearchResponse, InboxCreateParams,
    InboxItemResponse, InsightChatParams, InsightChatStarted, InsightEvolutionResponse,
    InsightQuizSubmitParams, InsightReviewResponse, InsightReviewStarted,
    InsightSaveFlashcardsParams, InsightVersionResponse, KnowledgeGrowthResponse, NoteCreateParams,
    NoteEditingFinishedParams, NoteLinkResponse, NoteListItem, NoteResponse,
    NoteRetentionHealthResponse, NoteSuggestionsResponse, NoteUpdateParams, NoteVersionResponse,
    NotebookCreateParams, NotebookResponse, NotebookUpdateParams, PersonaChatParams,
    PersonaChatResponse, PersonaResponse, RatePersonaParams, RecentLearningSession,
    ScenarioChallengeResponse, ScopePreviewParams, ScopePreviewResponse, SetPersonaPinsParams,
    StrugglingCardResponse, TabContent, UpdatePersonaParams,
};
use desktop_shared::{errors::ApiError, CommandResult};
use tauri::{Emitter, State};

use crate::app_core::AppCore;

/// Bridges `AppEventEmitter` to Tauri's `Emitter` trait (notes-local copy).
struct TauriEmitter(tauri::AppHandle);

impl ::app_core::events::AppEventEmitter for TauriEmitter {
    fn emit_event(&self, event_name: &str, payload: serde_json::Value) {
        let _ = self.0.emit(event_name, payload);
    }
}

// ── Note commands ───────────────────────────────────────────────────────

#[klynt_command]
pub async fn note_list(notebook_id: Option<String>) -> Vec<NoteListItem> {
    state.note_list(notebook_id).await
}

#[klynt_command]
pub async fn note_get(id: String) -> NoteResponse {
    state.note_get(id).await
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn note_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: NoteCreateParams,
) -> CommandResult<NoteResponse> {
    let (result, updates) = state.note_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn note_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: NoteUpdateParams,
) -> CommandResult<NoteResponse> {
    let (result, updates) = state.note_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn note_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> CommandResult<bool> {
    let (result, updates) = state.note_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn note_search(query: String) -> Vec<NoteListItem> {
    state.note_search(query).await
}

#[klynt_command]
pub async fn note_search_semantic(query: String) -> Vec<NoteListItem> {
    state.note_search_semantic(&query).await
}

#[klynt_command]
pub async fn note_search_hybrid(query: String) -> HybridSearchResponse {
    state.note_search_hybrid(&query).await
}

#[klynt_command]
pub async fn note_links_all() -> Vec<NoteLinkResponse> {
    state.note_links_all().await
}

#[klynt_command]
pub async fn note_list_by_entity(
    entity_type: String,
    entity_id: String,
) -> Vec<NoteListItem> {
    state.note_list_by_entity(entity_type, entity_id).await
}

// ── Version commands ────────────────────────────────────────────────────

#[klynt_command]
pub async fn note_version_list(note_id: String) -> Vec<NoteVersionResponse> {
    state.note_version_list(note_id).await
}

#[klynt_command]
pub async fn note_version_create(note_id: String) -> NoteVersionResponse {
    state.note_version_create(note_id).await
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn note_version_restore(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    version_id: String,
    note_id: String,
) -> CommandResult<NoteResponse> {
    let (result, updates) = state.note_version_restore(version_id, note_id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

// ── Attachment commands ─────────────────────────────────────────────────

#[klynt_command]
pub async fn note_save_attachment(
    data: String,
    filename: String,
) -> String {
    state.note_save_attachment(data, filename).await
}

// ── Notebook commands ───────────────────────────────────────────────────

#[klynt_command]
pub async fn notebook_list() -> Vec<NotebookResponse> {
    state.notebook_list().await
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn notebook_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: NotebookCreateParams,
) -> CommandResult<NotebookResponse> {
    let (result, updates) = state.notebook_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn notebook_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: NotebookUpdateParams,
) -> CommandResult<NotebookResponse> {
    let (result, updates) = state.notebook_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn notebook_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> CommandResult<bool> {
    let (result, updates) = state.notebook_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

// ── Archive commands ───────────────────────────────────────────────────

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn note_archive(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> CommandResult<()> {
    let (_, updates) = state.note_archive(&id).await?;
    super::emit_updates(&app, &updates);
    Ok(())
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn note_unarchive(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> CommandResult<()> {
    let (_, updates) = state.note_unarchive(&id).await?;
    super::emit_updates(&app, &updates);
    Ok(())
}

#[klynt_command]
pub async fn note_list_archived() -> Vec<NoteListItem> {
    state.note_list_archived().await
}

// ── Backlink commands ─────────────────────────────────────────────────

#[klynt_command]
pub async fn note_backlinks(id: String) -> Vec<BacklinkResponse> {
    state.note_backlinks(&id).await
}

// ── Suggestion commands ───────────────────────────────────────────────

#[klynt_command]
pub async fn note_suggestions(id: String) -> NoteSuggestionsResponse {
    state.note_suggestions(&id).await
}

// ── Tag commands ──────────────────────────────────────────────────────

#[klynt_command]
pub async fn note_tags_all() -> Vec<(String, i64)> {
    state
        .note_repo
        .get_all_tags()
        .await
        .map_err(|e| ApiError::new("STORAGE", e.to_string()))
}

// ── Unlinked mentions ─────────────────────────────────────────────────

#[klynt_command]
pub async fn note_unlinked_mentions(id: String) -> Vec<NoteResponse> {
    state.note_unlinked_mentions(&id).await
}

// ── Inbox commands ────────────────────────────────────────────────────

#[klynt_command]
pub async fn inbox_create(params: InboxCreateParams) -> InboxItemResponse {
    state.inbox_create(&params.content).await
}

#[klynt_command]
pub async fn inbox_list() -> Vec<InboxItemResponse> {
    state.inbox_list().await
}

#[klynt_command]
pub async fn inbox_delete(id: String) -> () {
    state.inbox_delete(&id).await
}

// ── Insight Review commands ─────────────────────────────────────────

#[klynt_command]
pub async fn note_insight_review(
    note_id: String,
    scope_config: Option<desktop_shared::commands::InsightScopeConfigParams>,
    squad_id: Option<String>,
) -> InsightReviewStarted {
    state
        .note_insight_review(&note_id, scope_config.as_ref(), squad_id.as_deref(), None)
        .await
}

#[klynt_command]
pub async fn note_insight_cache_get(
    note_id: String,
) -> Option<InsightReviewResponse> {
    state.note_insight_cache_get(&note_id).await
}

#[klynt_command]
pub async fn note_insight_save_flashcards(
    params: InsightSaveFlashcardsParams,
) -> Vec<FlashcardResponse> {
    state.insight_save_flashcards(params).await
}

#[klynt_command]
pub async fn note_insight_submit_quiz(params: InsightQuizSubmitParams) -> () {
    state.note_insight_submit_quiz(&params).await
}

#[klynt_command]
pub async fn note_insight_regenerate_tab(note_id: String, tab: String) -> TabContent {
    state.note_insight_regenerate_tab(&note_id, &tab).await
}

#[klynt_command]
pub async fn note_insight_debate(note_id: String, squad_id: Option<String>) -> () {
    state
        .note_insight_debate(&note_id, squad_id.as_deref())
        .await
}

#[klynt_command]
pub async fn note_insight_list_versions(note_id: String) -> Vec<InsightVersionResponse> {
    state.note_insight_list_versions(&note_id).await
}

#[klynt_command]
pub async fn note_insight_get_evolution(note_id: String) -> InsightEvolutionResponse {
    state.note_insight_get_evolution(&note_id).await
}

#[klynt_command]
pub async fn note_insight_get_version(insight_id: String) -> InsightReviewResponse {
    state.note_insight_get_version(&insight_id).await
}

#[klynt_command]
pub async fn note_insight_generate_scenario(note_id: String) -> ScenarioChallengeResponse {
    state.note_insight_generate_scenario(&note_id).await
}

#[klynt_command]
pub async fn note_insight_changes_summary(
    note_id: String,
) -> Option<ChangesSummaryResponse> {
    state.note_insight_changes_summary(&note_id).await
}

#[klynt_command]
pub async fn note_insight_knowledge_growth(days: Option<u32>) -> KnowledgeGrowthResponse {
    state.note_insight_knowledge_growth(days.unwrap_or(7)).await
}

// ── Persona Management commands ───────────────────────────────────

#[klynt_command]
pub async fn note_insight_list_personas() -> Vec<PersonaResponse> {
    state.note_insight_list_personas().await
}

#[klynt_command]
pub async fn note_insight_create_persona(params: CreatePersonaParams) -> PersonaResponse {
    state.note_insight_create_persona(params).await
}

#[klynt_command]
pub async fn note_insight_update_persona(params: UpdatePersonaParams) -> PersonaResponse {
    state.note_insight_update_persona(params).await
}

#[klynt_command]
pub async fn note_insight_delete_persona(id: String) -> () {
    state.note_insight_delete_persona(&id).await
}

#[klynt_command]
pub async fn note_insight_toggle_persona(id: String, active: bool) -> () {
    state.note_insight_toggle_persona(&id, active).await
}

#[klynt_command]
pub async fn note_insight_set_pins(params: SetPersonaPinsParams) -> () {
    state.note_insight_set_pins(params).await
}

#[klynt_command]
pub async fn note_insight_rate_persona(params: RatePersonaParams) -> () {
    state.note_insight_rate_persona(params).await
}

#[klynt_command]
pub async fn note_insight_auto_generate_persona(note_id: String) -> PersonaResponse {
    state.note_insight_auto_generate_persona(&note_id).await
}

#[klynt_command]
pub async fn note_insight_persona_chat(params: PersonaChatParams) -> PersonaChatResponse {
    state.note_insight_persona_chat(&params).await
}

#[klynt_command]
pub async fn note_insight_preview_scope(params: ScopePreviewParams) -> ScopePreviewResponse {
    state.note_insight_preview_scope(params).await
}

// ── Flashcard Review commands ───────────────────────────────────

#[klynt_command]
pub async fn flashcard_list_decks() -> Vec<DeckSummaryResponse> {
    state.flashcard_list_decks().await
}

#[klynt_command]
pub async fn flashcard_get_due(deck: String, limit: Option<i64>) -> Vec<FlashcardResponse> {
    state.flashcard_get_due(&deck, limit.unwrap_or(10)).await
}

#[klynt_command]
pub async fn flashcard_record_review(params: FlashcardReviewParams) -> FlashcardResponse {
    state.flashcard_record_review(params).await
}

#[klynt_command]
pub async fn flashcard_get(id: String) -> FlashcardResponse {
    state.flashcard_get(&id).await
}

#[klynt_command]
pub async fn flashcard_create(params: FlashcardCreateParams) -> FlashcardResponse {
    state.flashcard_create(params).await
}

#[klynt_command]
pub async fn flashcard_update(params: FlashcardUpdateParams) -> FlashcardResponse {
    state.flashcard_update(params).await
}

#[klynt_command]
pub async fn flashcard_list_cards(params: FlashcardListParams) -> Vec<FlashcardResponse> {
    state.flashcard_list_cards(params).await
}

#[klynt_command]
pub async fn flashcard_delete(id: String) -> bool {
    state.flashcard_delete(&id).await
}

#[klynt_command]
pub async fn flashcard_get_all_due(limit: i64) -> Vec<FlashcardResponse> {
    state.flashcard_get_all_due(limit).await
}

#[klynt_command]
pub async fn flashcard_total_due() -> i64 {
    state.flashcard_total_due().await
}

#[klynt_command]
pub async fn flashcard_list_struggling(
    limit: Option<i64>,
) -> Vec<StrugglingCardResponse> {
    state.flashcard_list_struggling(limit.unwrap_or(5)).await
}

#[klynt_command]
pub async fn flashcard_generate(
    params: FlashcardGenerateParams,
) -> FlashcardGenerateResponse {
    state.flashcard_generate(params).await
}

#[klynt_command]
pub async fn flashcard_save_generated(
    params: FlashcardSaveGeneratedParams,
) -> Vec<FlashcardResponse> {
    state.flashcard_save_generated(params).await
}

// ── Active Recall commands ──────────────────────────────────────────

#[klynt_command]
pub async fn flashcard_submit_answer(
    params: FlashcardSubmitAnswerParams,
) -> GradeResultResponse {
    state.flashcard_submit_answer(params).await
}

#[klynt_command]
pub async fn flashcard_explain_answer(
    params: FlashcardExplainParams,
) -> FlashcardExplainResponse {
    state.flashcard_explain_answer(params).await
}

#[klynt_command]
pub async fn flashcard_generate_distractors(
    params: FlashcardDistractorParams,
) -> FlashcardDistractorResponse {
    state.flashcard_generate_distractors(params).await
}

#[klynt_command]
pub async fn flashcard_save_mode_preference(deck: String, mode: String) -> () {
    state
        .deck_preference_repo()?
        .set(&deck, &mode)
        .await
        .map_err(|e| ApiError::new("DB_ERROR", e.to_string()))?;
    Ok(())
}

#[klynt_command]
pub async fn flashcard_get_mode_preference(
    deck: String,
) -> Option<DeckPreferenceResponse> {
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

#[klynt_command]
pub async fn flashcard_get_prerequisites(card_id: String) -> Vec<FlashcardResponse> {
    state.flashcard_get_prerequisites(&card_id).await
}

#[klynt_command]
pub async fn flashcard_save_session(
    params: desktop_shared::commands::ReviewSessionSaveParams,
) -> () {
    state.flashcard_save_session(params).await
}

#[klynt_command]
pub async fn flashcard_recent_learning_sessions(
    limit: Option<usize>,
) -> Vec<RecentLearningSession> {
    state
        .flashcard_recent_learning_sessions(limit.unwrap_or(3))
        .await
}

// ── Retention Health ────────────────────────────────────────────────

#[klynt_command]
pub async fn note_retention_health(
    note_id: String,
) -> Option<NoteRetentionHealthResponse> {
    state.note_retention_health(note_id).await
}

// ── Editing finished command ─────────────────────────────────────────

#[klynt_command]
pub async fn note_editing_finished(params: NoteEditingFinishedParams) -> () {
    state.note_editing_finished(params).await
}

// ── Insight Tab Chat commands ─────────────────────────────────────────

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn note_insight_tab_chat(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppCore>>,
    params: InsightChatParams,
) -> CommandResult<InsightChatStarted> {
    let emitter: Arc<dyn ::app_core::events::AppEventEmitter> = Arc::new(TauriEmitter(app));
    state.note_insight_tab_chat(&params, emitter).await
}

#[klynt_command]
pub async fn note_insight_clear_tab_chats(note_id: String) -> () {
    state.note_insight_clear_tab_chats(&note_id).await
}

// ── Import / Export ──────────────────────────────────────────────

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn note_import_files(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: desktop_shared::commands::NoteImportParams,
) -> CommandResult<desktop_shared::commands::NoteImportResult> {
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

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn note_export(
    state: State<'_, Arc<AppCore>>,
    _app: tauri::AppHandle,
    params: desktop_shared::commands::NoteExportParams,
) -> CommandResult<desktop_shared::commands::NoteExportResult> {
    state.note_export(params).await
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
        // note_insight_tab_chat is handled inline in dev_server/dispatch.rs (needs SSE channels)
        "note_insight_clear_tab_chats" => {
            let note_id = try_field!(dev::get_str(body, "noteId"));
            dev::val(core.note_insight_clear_tab_chats(&note_id).await)
        }
        _ => return None,
    })
}
