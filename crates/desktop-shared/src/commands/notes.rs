use serde::{Deserialize, Serialize};

// ── Notes ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteCreateParams {
    pub title: String,
    pub notebook_id: Option<String>,
    pub body: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteUpdateParams {
    pub id: String,
    pub title: Option<String>,
    pub body: Option<String>,
    pub body_html: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteLinkResponse {
    pub source_id: String,
    pub target_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteVersionResponse {
    pub id: String,
    pub note_id: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookCreateParams {
    pub title: String,
    pub parent_id: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
}

// ── Hybrid search ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HybridSearchResponse {
    pub exact: Vec<NoteResponse>,
    pub related: Vec<NoteResponse>,
}

// ── Inbox ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxCreateParams {
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxItemResponse {
    pub id: String,
    pub content: String,
    pub status: String,
    pub created_at: String,
}

// ── Suggestions ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSuggestionsResponse {
    pub related_notes: Vec<ScoredNoteResponse>,
    pub link_suggestions: Vec<LinkSuggestionResponse>,
    pub suggested_tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoredNoteResponse {
    pub note: NoteResponse,
    pub score: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkSuggestionResponse {
    pub note: NoteResponse,
    pub score: f64,
    pub reason: String,
}

// ── Backlinks ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BacklinkResponse {
    pub note: NoteResponse,
    pub context: Option<String>,
}

// ── Insight Review ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightReviewStarted {
    pub insight_review_id: String,
    pub content_hash: String,
    pub cached: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub persona_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabContent {
    pub tab: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashcardResponse {
    pub id: String,
    pub deck: String,
    pub question: String,
    pub answer: String,
    pub card_type: String,
    pub choices: Option<serde_json::Value>,
    pub stability: f64,
    pub difficulty: f64,
    pub due_at: Option<String>,
    pub state: String,
    pub review_count: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightSaveFlashcardsParams {
    pub note_id: String,
    pub insight_review_id: String,
    pub deck_name: String,
    pub questions: Vec<QuizQuestion>,
}

// ── Insight Quiz ─────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightQuizSubmitParams {
    pub insight_review_id: String,
    pub score: f64,
    pub total: i32,
}

// ── Insight Versions ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightVersionResponse {
    pub id: String,
    pub version: i64,
    pub generated_at: String,
    pub input_hash: String,
    pub has_parent: bool,
}

// ── Insight Evolution ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightEvolutionResponse {
    pub note_id: String,
    pub note_title: String,
    pub versions: Vec<InsightEvolutionPoint>,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioChallengeResponse {
    pub title: String,
    pub situation: String,
    pub questions: Vec<String>,
    pub model_answer: String,
    pub source_notes: Vec<String>,
    pub difficulty_score: f64,
}

// ── Insight Scope Config ─────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
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

// ── Persona Management ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaResponse {
    pub id: String,
    pub name: String,
    pub role: String,
    pub expertise: String,
    pub perspective: String,
    pub tone: String,
    pub icon: String,
    pub source: String,
    pub domains: Vec<String>,
    pub is_active: bool,
    pub relevance_score: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePersonaParams {
    pub name: String,
    pub role: String,
    pub expertise: String,
    pub perspective: String,
    pub tone: String,
    pub icon: String,
    pub domains: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePersonaParams {
    pub id: String,
    pub name: Option<String>,
    pub role: Option<String>,
    pub expertise: Option<String>,
    pub perspective: Option<String>,
    pub tone: Option<String>,
    pub icon: Option<String>,
    pub domains: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPersonaPinsParams {
    pub note_id: String,
    pub persona_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RatePersonaParams {
    pub id: String,
    pub helpful: bool,
}
