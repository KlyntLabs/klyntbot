use serde::{Deserialize, Serialize};

// ── Annotation CRUD ─────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationCreateParams {
    pub note_id: String,
    pub mark_id: String,
    pub content: String,
    pub quoted_text: Option<String>,
    pub range_start: Option<i64>,
    pub range_end: Option<i64>,
    pub ai_suggestion: Option<String>,
    pub tags: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationUpdateParams {
    pub id: String,
    pub content: Option<String>,
    pub tags: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationResponse {
    pub id: String,
    pub note_id: String,
    pub mark_id: Option<String>,
    pub content: String,
    pub quoted_text: Option<String>,
    pub range_start: Option<i64>,
    pub range_end: Option<i64>,
    pub ai_suggestion: Option<String>,
    pub tags: String,
    pub created_at: String,
    pub updated_at: String,
}

// ── Linked Context ──────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LinkedContextParams {
    pub note_id: String,
    pub section_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LinkedContextResponse {
    pub semantic_facts: Vec<LinkedFact>,
    pub episodic_memories: Vec<LinkedMemory>,
    pub related_annotations: Vec<AnnotationResponse>,
    pub procedural_rules: Vec<LinkedRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LinkedFact {
    pub id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub source_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LinkedMemory {
    pub id: String,
    pub content: String,
    pub domain: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LinkedRule {
    pub id: String,
    pub rule_text: String,
    pub domain: String,
    pub signal_count: i64,
}

// ── AI Suggestion ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AiSuggestionResponse {
    pub suggestion: Option<String>,
    pub confidence: f64,
    pub related_fact_ids: Vec<String>,
}
