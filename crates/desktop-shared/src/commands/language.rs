use serde::{Deserialize, Serialize};

// ── Translation Breakdown ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslateBreakdownParams {
    pub text: String,
    pub source_lang: String,
    pub target_lang: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TranslateBreakdownResponse {
    pub translation: String,
    pub words: Vec<WordBreakdown>,
    pub grammar_patterns: Vec<GrammarPattern>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WordBreakdown {
    pub word: String,
    pub reading: Option<String>,
    pub meaning: String,
    pub part_of_speech: String,
    pub proficiency_level: Option<String>,
    pub example_sentence: Option<String>,
    #[serde(default)]
    pub is_new: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrammarPattern {
    pub pattern: String,
    pub explanation: String,
    pub pattern_type: Option<String>,
}

// ── Translation Evaluation ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateTranslationParams {
    pub source_text: String,
    pub user_translation: String,
    pub source_lang: String,
    pub target_lang: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TranslationEvalResponse {
    pub grades: EvalGrades,
    pub corrections: Vec<Correction>,
    pub model_translation: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EvalGrades {
    pub meaning: String,
    pub grammar: String,
    pub naturalness: String,
    pub word_choice: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Correction {
    pub original: String,
    pub suggested: String,
    pub explanation: String,
    pub category: String,
}

// ── Vocabulary Save ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VocabularySaveParams {
    pub words: Vec<VocabItem>,
    pub note_id: Option<String>,
    pub deck: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VocabItem {
    pub word: String,
    pub reading: Option<String>,
    pub meaning: String,
    pub part_of_speech: String,
    pub example_sentence: Option<String>,
}

// ── Confusable Detection ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectConfusablesParams {
    pub word: String,
    pub source_lang: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConfusableResponse {
    pub has_confusable: bool,
    pub confusable_word: Option<String>,
    pub confusable_meaning: Option<String>,
    pub explanation: Option<String>,
}

// ── Annotation Enrichment ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrichAnnotationParams {
    pub annotation_id: String,
    pub quoted_text: String,
    pub source_lang: String,
    pub target_lang: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationEnrichmentResponse {
    pub translation: String,
    pub words: Vec<WordBreakdown>,
}
