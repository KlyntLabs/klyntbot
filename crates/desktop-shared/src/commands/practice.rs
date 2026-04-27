use serde::{Deserialize, Serialize};

// ── Segment Extraction ────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PracticeSegmentParams {
    pub note_id: String,
    pub source_lang: String,
    pub target_lang: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PracticeSegment {
    pub index: u32,
    pub text: String,
    #[serde(alias = "type")]
    pub segment_type: String,
    #[serde(alias = "suggested_focus")]
    pub suggested_focus: String,
    #[serde(default)]
    pub skipped: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PracticeSegmentResponse {
    pub segments: Vec<PracticeSegment>,
    pub estimated_mins: u32,
    pub cached_at: Option<String>,
}

// ── Session Start ─────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PracticeStartParams {
    pub note_id: String,
    pub segments: Vec<PracticeSegment>,
    pub source_lang: String,
    pub target_lang: String,
    #[serde(default)]
    pub start_index: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PracticeSessionResponse {
    pub id: String,
    pub note_id: String,
    pub source_lang: String,
    pub target_lang: String,
    pub status: String,
    pub segments: String,
    pub current_index: u32,
    pub results: String,
    pub user_translation_doc: Option<String>,
    pub average_score: Option<f64>,
    pub started_at: String,
    pub completed_at: Option<String>,
}

// ── Submit Translation ────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PracticeSubmitParams {
    pub session_id: String,
    pub index: u32,
    pub user_translation: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PracticeEvalResponse {
    #[serde(alias = "overall_grade")]
    pub overall_grade: String,
    pub scores: PracticeScores,
    pub corrections: Vec<PracticeCorrection>,
    #[serde(alias = "model_translation")]
    pub model_translation: String,
    pub encouragement: String,
    #[serde(alias = "improvement_hint")]
    pub improvement_hint: Option<String>,
    #[serde(default, alias = "coaching_nudge")]
    pub coaching_nudge: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PracticeScores {
    pub meaning: String,
    pub grammar: String,
    pub naturalness: String,
    #[serde(alias = "word_choice", alias = "wordChoice")]
    pub word_choice: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PracticeCorrection {
    pub original: String,
    pub suggested: String,
    pub explanation: String,
}

// ── Confirm Segment ───────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PracticeConfirmParams {
    pub session_id: String,
    pub index: u32,
    pub final_translation: String,
    pub confidence_rating: u8,
    pub edited: bool,
    pub overall_grade: String,
    #[serde(default)]
    pub scores_json: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PracticeConfirmResponse {
    pub next_index: u32,
    pub is_complete: bool,
}

// ── Get / List Sessions ───────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PracticeGetParams {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub note_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PracticeListParams {
    pub note_id: String,
}

// ── Complete Session ──────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PracticeCompleteParams {
    pub session_id: String,
    pub save_to_sr: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PracticeCompleteResponse {
    pub average_score: f64,
    pub weak_unit_count: u32,
    pub flashcards_created: u32,
}
