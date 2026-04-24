use ai_core_macros::AiEntity;
use serde::{Deserialize, Serialize};

/// A single card produced by the LLM card generator.
/// Deserialized from the LLM JSON response, then mapped to `NewFlashcard` for persistence.
#[derive(Debug, Clone, Serialize, Deserialize, AiEntity)]
#[ai(
    entity_type = "generated_card",
    embed_on = ["front", "back", "source_context"]
)]
pub struct GeneratedCard {
    pub front: String,
    pub back: String,
    pub card_type: String,
    pub tags: Vec<String>,
    pub source_context: Option<String>,
    pub cloze_data: Option<serde_json::Value>,
    pub vocab_data: Option<serde_json::Value>,
    pub difficulty_estimate: Option<i32>,
    pub prerequisite_concepts: Option<Vec<String>>,
}

/// Context assembled for card generation — passed to the prompt builder.
pub struct CardGenerationContext {
    pub note_content: String,
    pub note_title: String,
    pub existing_cards_summary: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_core::AiEntity;

    #[test]
    fn generated_card_ai_entity_joins_nonempty_fields() {
        let c = GeneratedCard {
            front: "What is 2+2?".into(),
            back: "4".into(),
            card_type: "basic".into(),
            tags: vec![],
            source_context: Some("arithmetic".into()),
            cloze_data: None,
            vocab_data: None,
            difficulty_estimate: None,
            prerequisite_concepts: None,
        };
        assert_eq!(GeneratedCard::entity_type(), "generated_card");
        assert_eq!(c.embed_text(), "What is 2+2?\n4\narithmetic");
    }

    #[test]
    fn generated_card_ai_entity_skips_empty_option() {
        let c = GeneratedCard {
            front: "f".into(),
            back: "b".into(),
            card_type: "basic".into(),
            tags: vec![],
            source_context: None,
            cloze_data: None,
            vocab_data: None,
            difficulty_estimate: None,
            prerequisite_concepts: None,
        };
        assert_eq!(c.embed_text(), "f\nb");
    }
}
