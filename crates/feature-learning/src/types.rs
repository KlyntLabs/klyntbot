use serde::{Deserialize, Serialize};

/// A single card produced by the LLM card generator.
/// Deserialized from the LLM JSON response, then mapped to `NewFlashcard` for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedCard {
    pub front: String,
    pub back: String,
    pub card_type: String,
    pub tags: Vec<String>,
    pub source_context: Option<String>,
    pub cloze_data: Option<serde_json::Value>,
    pub vocab_data: Option<serde_json::Value>,
}

/// Context assembled for card generation — passed to the prompt builder.
pub struct CardGenerationContext {
    pub note_content: String,
    pub note_title: String,
    pub existing_cards_summary: Option<String>,
}
