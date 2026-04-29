use serde::{Deserialize, Serialize};

// ── Notes ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NoteResponse {
    pub id: String,
    pub notebook_id: Option<String>,
    pub title: String,
    pub body: String,
    pub body_html: Option<String>,
    pub pinned: bool,
    pub archived: bool,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub split_content: Option<String>,
    pub split_mode: Option<String>,
    pub perspective_config: Option<String>,
    pub last_visited_at: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Lightweight note for list views — excludes body, HTML, split/perspective data.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NoteListItem {
    pub id: String,
    pub notebook_id: Option<String>,
    pub title: String,
    pub pinned: bool,
    pub archived: bool,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NoteCreateParams {
    pub title: String,
    pub notebook_id: Option<String>,
    pub body: Option<String>,
    pub body_json: Option<String>,
    pub tags: Option<Vec<String>>,
    pub created_at: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NoteEditingFinishedParams {
    pub note_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NoteUpdateParams {
    pub id: String,
    pub title: Option<String>,
    pub body: Option<String>,
    pub body_html: Option<String>,
    pub body_json: Option<String>,
    pub pinned: Option<bool>,
    /// `None` = don't change, `Some(None)` = move to root, `Some(Some(id))` = move to folder
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub notebook_id: Option<Option<String>>,
    pub tags: Option<Vec<String>>,
    /// `None` = don't change, `Some(None)` = clear icon, `Some(Some(emoji))` = set icon
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub icon: Option<Option<String>>,
    /// `None` = don't change, `Some(None)` = clear color, `Some(Some(hex))` = set color
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub color: Option<Option<String>>,
    /// `None` = don't change, `Some(None)` = clear split_content, `Some(Some(json))` = set split_content
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub split_content: Option<Option<String>>,
    /// `None` = don't change, `Some(None)` = clear split_mode, `Some(Some(mode))` = set split_mode
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub split_mode: Option<Option<String>>,
    /// `None` = don't change, `Some(None)` = clear perspective_config, `Some(Some(json))` = set perspective_config
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub perspective_config: Option<Option<String>>,
}

/// Deserializes a field that distinguishes between absent, null, and present.
/// - absent → `None` (don't change)
/// - `null` → `Some(None)` (set to null / move to root)
/// - `"value"` → `Some(Some("value"))` (set to value)
fn deserialize_nullable_field<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NotebookUpdateParams {
    pub id: String,
    pub title: Option<String>,
    /// `None` = don't change, `Some(None)` = clear icon, `Some(Some(name))` = set icon
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub icon: Option<Option<String>>,
    /// `None` = don't change, `Some(None)` = clear color, `Some(Some(hex))` = set color
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub color: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_nullable_field")]
    pub parent_id: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NotebookResponse {
    pub id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub sort_order: i32,
    pub note_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NoteLinkResponse {
    pub source_id: String,
    pub target_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NoteVersionResponse {
    pub id: String,
    pub note_id: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NotebookCreateParams {
    pub title: String,
    pub parent_id: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
}

// ── Hybrid search ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HybridSearchResponse {
    pub exact: Vec<NoteResponse>,
    pub related: Vec<NoteResponse>,
}

// ── Inbox ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InboxCreateParams {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InboxItemResponse {
    pub id: String,
    pub content: String,
    pub status: String,
    pub created_at: String,
}

// ── Suggestions ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NoteSuggestionsResponse {
    pub related_notes: Vec<ScoredNoteResponse>,
    pub link_suggestions: Vec<LinkSuggestionResponse>,
    pub suggested_tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ScoredNoteResponse {
    pub note: NoteResponse,
    pub score: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LinkSuggestionResponse {
    pub note: NoteResponse,
    pub score: f64,
    pub reason: String,
}

// ── Backlinks ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BacklinkResponse {
    pub note: NoteResponse,
    pub context: Option<String>,
}

// ── Insight Review ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InsightReviewStarted {
    pub insight_review_id: String,
    pub content_hash: String,
    pub cached: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InsightReviewResponse {
    pub insight_review_id: String,
    pub note_id: String,
    pub version: i64,
    pub generated_at: String,
    pub synthesis: Option<String>,
    pub gap_analysis: Option<String>,
    pub self_assessment: Option<Vec<QuizQuestion>>,
    pub concept_map: Option<String>,
    pub perspectives: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct QuizQuestion {
    pub id: String,
    #[serde(rename = "type")]
    pub question_type: String,
    pub question: String,
    pub choices: Option<Vec<String>>,
    pub correct_answer: String,
    pub explanation: String,
    pub source_notes: Vec<String>,
    pub difficulty: String,
    pub difficulty_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TabContent {
    pub tab: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardResponse {
    pub id: String,
    pub deck: String,
    pub front: String,
    pub back: String,
    pub card_type: String,
    #[specta(type = Option<crate::specta_helpers::JsonValue>)]
    pub cloze_data: Option<serde_json::Value>,
    #[specta(type = Option<crate::specta_helpers::JsonValue>)]
    pub vocab_data: Option<serde_json::Value>,
    #[specta(type = Option<crate::specta_helpers::JsonValue>)]
    pub image_data: Option<serde_json::Value>,
    #[specta(type = crate::specta_helpers::JsonValue)]
    pub tags: serde_json::Value,
    pub source_note_id: Option<String>,
    pub source_context: Option<String>,
    pub stability: f64,
    pub difficulty: f64,
    pub due_at: Option<String>,
    pub state: String,
    pub review_count: i64,
    pub recall_speed_ms: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InsightSaveFlashcardsParams {
    pub note_id: String,
    pub insight_review_id: String,
    pub deck_name: String,
    pub questions: Vec<QuizQuestion>,
}

// ── Note Retention Health ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NoteRetentionHealthResponse {
    pub avg_stability: f64,
    pub total_cards: i64,
    pub total_lapses: i64,
    pub health_score: f64,
}

// ── Struggling Cards ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StrugglingCardResponse {
    pub id: String,
    pub front: String,
    pub back: String,
    pub deck: String,
    pub lapses: i64,
    pub review_count: i64,
    pub source_note_id: Option<String>,
}

// ── Insight Tab Chat ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InsightChatParams {
    pub note_id: String,
    pub tab_name: String,
    pub user_message: String,
    pub session_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InsightChatStarted {
    pub session_key: String,
    pub message_id: String,
}

// ── Flashcard Review ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DeckSummaryResponse {
    pub name: String,
    pub card_count: i64,
    pub due_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardReviewParams {
    pub card_id: String,
    pub quality: String, // "again" | "hard" | "good" | "easy"
    pub recall_speed_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardCreateParams {
    pub deck: String,
    pub front: String,
    pub back: String,
    pub card_type: String,
    pub tags: Option<Vec<String>>,
    pub source_note_id: Option<String>,
    #[specta(type = Option<crate::specta_helpers::JsonValue>)]
    pub cloze_data: Option<serde_json::Value>,
    #[specta(type = Option<crate::specta_helpers::JsonValue>)]
    pub vocab_data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardUpdateParams {
    pub id: String,
    pub front: String,
    pub back: String,
    pub deck: String,
    pub tags: Option<Vec<String>>,
    #[specta(type = Option<crate::specta_helpers::JsonValue>)]
    pub cloze_data: Option<serde_json::Value>,
    #[specta(type = Option<crate::specta_helpers::JsonValue>)]
    pub vocab_data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardListParams {
    pub deck: String,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ── Card Generation ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardGenerateParams {
    /// Generate from a specific note (fetches note content)
    pub note_id: Option<String>,
    /// Generate from raw text (clipboard, selection)
    pub text_content: Option<String>,
    /// Suggested deck name (optional)
    pub deck_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedCardPreview {
    pub front: String,
    pub back: String,
    pub card_type: String,
    pub tags: Vec<String>,
    pub source_context: Option<String>,
    #[specta(type = Option<crate::specta_helpers::JsonValue>)]
    pub cloze_data: Option<serde_json::Value>,
    #[specta(type = Option<crate::specta_helpers::JsonValue>)]
    pub vocab_data: Option<serde_json::Value>,
    pub difficulty_estimate: Option<i32>,
    pub prerequisite_concepts: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardGenerateResponse {
    pub cards: Vec<GeneratedCardPreview>,
    pub deck_suggestion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardSaveGeneratedParams {
    pub note_id: Option<String>,
    pub deck: String,
    pub cards: Vec<GeneratedCardPreview>,
}

// ── Insight Quiz ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InsightQuizSubmitParams {
    pub insight_review_id: String,
    pub score: f64,
    pub total: i32,
}

// ── Insight Versions ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InsightVersionResponse {
    pub id: String,
    pub version: i64,
    pub generated_at: String,
    pub input_hash: String,
    pub has_parent: bool,
}

// ── Insight Evolution ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InsightEvolutionResponse {
    pub note_id: String,
    pub note_title: String,
    pub versions: Vec<InsightEvolutionPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InsightEvolutionPoint {
    pub version: i64,
    pub generated_at: String,
    pub flashcard_success: f64,
    pub semantic_drift: f64,
    pub gap_closure: f64,
    pub quiz_score: f64,
    pub overall_progress: f64,
    pub change_note: String,
}

// ── Scenario Challenge ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioChallengeResponse {
    pub title: String,
    pub situation: String,
    pub questions: Vec<String>,
    pub model_answer: String,
    pub source_notes: Vec<String>,
    pub difficulty_score: f64,
}

// ── Changes Summary ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangesSummaryResponse {
    pub summary: String,
}

// ── Knowledge Growth ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeGrowthResponse {
    pub new_facts_count: usize,
    pub updated_facts_count: usize,
    pub superseded_facts_count: usize,
    pub by_domain: Vec<DomainCount>,
    pub period_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DomainCount {
    pub domain: String,
    pub count: usize,
}

// ── Insight Scope Preview ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ScopePreviewResponse {
    pub notes: Vec<ScopePreviewNote>,
    pub links: Vec<ScopePreviewLink>,
    /// Summary of what context the AI will see.
    pub context_summary: ContextSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ScopePreviewNote {
    pub id: String,
    pub title: String,
    pub notebook_id: Option<String>,
    /// Approximate word count of the note body (0 = empty).
    pub word_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ContextSummary {
    pub total_notes: u32,
    pub total_words: u32,
    pub strong_atoms: u32,
    pub fading_atoms: u32,
    /// Number of semantic facts included (when cognitive context is on).
    pub facts_count: u32,
    /// Number of episodic memories included (when cognitive context is on).
    pub memories_count: u32,
    /// Number of entity connections (when deep dive is on).
    pub entity_count: u32,
    /// Whether cognitive context is included.
    pub include_cognitive: bool,
    /// Whether deep dive is included.
    pub deep_dive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ScopePreviewLink {
    pub source_id: String,
    pub target_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ScopePreviewParams {
    pub note_id: String,
    #[serde(flatten)]
    pub scope: InsightScopeConfigParams,
}

// ── Insight Scope Config ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InsightScopeConfigParams {
    #[serde(default)]
    pub scope_type: Option<String>,
    pub radius: Option<f64>,
    #[serde(default)]
    pub node_ids: Option<Vec<String>>,
    pub include_cognitive: Option<bool>,
    pub deep_dive: Option<bool>,
    pub merge_threshold: Option<f64>,
}

// ── Distractor Generation ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardDistractorParams {
    pub card_id: String,
    #[serde(default = "default_distractor_count")]
    pub count: usize,
}

fn default_distractor_count() -> usize {
    3
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardDistractorResponse {
    pub distractors: Vec<String>,
    pub cached: bool,
}

// ── Active Recall Grading ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardSubmitAnswerParams {
    pub card_id: String,
    pub user_answer: String,
    pub mode: String, // "typed" | "voice" | "cloze_fill"
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GradeResultResponse {
    pub score: Option<f64>,
    pub suggested_rating: String,
    pub grading_method: String,
    pub explanation: Option<String>,
    pub diff_highlights: Vec<DiffSegmentResponse>,
    pub expected_answer: String,
    pub coaching_nudge: Option<String>,
    pub socratic_suggestion: Option<String>,
    pub key_concepts_present: Vec<String>,
    pub key_concepts_missing: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DiffSegmentResponse {
    pub text: String,
    pub status: String, // "match" | "missing" | "extra" | "partial"
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardExplainParams {
    pub card_id: String,
    pub user_answer: String,
    pub grade_explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardExplainResponse {
    pub explanation: String,
    pub saved_as_memory: bool,
}

// ── Deck Preference ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DeckPreferenceResponse {
    pub deck: String,
    pub answer_mode: String,
}

// ── Recent Learning Sessions ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RecentLearningSession {
    pub session_key: String,
    pub title: String,
    pub updated_at: String,
    pub preview: String,
}

// ── Review Session ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSessionSaveParams {
    pub session_id: String,
    pub cards_reviewed: i32,
    pub avg_score: f64,
    pub duration_seconds: i32,
    pub modes_used: Vec<String>,
    pub propagation_count: i32,
    pub weak_card_ids: Vec<String>,
    pub session_data: String,
    /// "completed" | "abandoned"
    pub status: String,
}

// ── Import / Export ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NoteImportParams {
    pub paths: Vec<String>,
    pub notebook_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NoteImportResult {
    pub imported: u32,
    pub skipped: Vec<SkippedFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SkippedFile {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NoteExportParams {
    pub note_ids: Option<Vec<String>>,
    pub notebook_ids: Option<Vec<String>>,
    pub destination: String,
    pub output_filename: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NoteExportResult {
    pub exported: u32,
}
