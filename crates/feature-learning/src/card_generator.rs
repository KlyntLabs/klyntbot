use crate::types::{CardGenerationContext, GeneratedCard};
use common::helpers::strip_llm_fences;
use common::truncate_chars;
use tracing::warn;

/// Build the system + user messages for card generation.
///
/// Returns `(system_prompt, user_prompt)` — caller sends both to the LLM.
pub fn build_generation_prompt(ctx: &CardGenerationContext) -> (String, String) {
    let system = r#"You are a flashcard generation assistant for spaced repetition learning. Generate high-quality, self-contained flashcards from the provided content.

Rules:
1. Generate 5-15 cards depending on content density
2. Use varied card types:
   - "basic" for concept questions and understanding checks (most common)
   - "cloze" for definitions, key facts, and fill-in-the-blank — use {{c1::hidden}} syntax in the front field
   - "vocabulary" for foreign language words — populate vocab_data with word, reading, meaning, example_sentence, part_of_speech
3. Each card MUST be self-contained (understandable without the source note)
4. Prefer testing understanding and application over rote memorization
5. source_context is a 1-2 sentence excerpt from the note that the card tests — include it for every card
6. Tags: 1-3 lowercase hyphenated concepts (e.g., "machine-learning", "te-form", "photosynthesis")
7. For cloze cards: front contains the text with {{c1::hidden}} markers, back is the full revealed text
8. For vocabulary cards: front is the word (with reading if applicable), back is the meaning + example

Respond ONLY with a JSON array. No markdown fences, no explanation, no preamble."#.to_string();

    let mut user = String::new();

    if let Some(ref existing) = ctx.existing_cards_summary {
        user.push_str("The user already has these flashcards from this note. Do NOT generate duplicates:\n");
        user.push_str(existing);
        user.push_str("\n\n");
    }

    user.push_str(&format!("--- BEGIN NOTE: {} ---\n", ctx.note_title));
    user.push_str(&ctx.note_content);
    user.push_str("\n--- END NOTE ---");

    (system, user)
}

/// Parse the LLM response JSON into a list of generated cards.
///
/// Handles common LLM quirks: markdown fences around JSON, trailing commas (via serde),
/// and individual card validation (skips malformed cards rather than failing entirely).
pub fn parse_generated_cards(response: &str) -> Result<Vec<GeneratedCard>, String> {
    let cleaned = strip_llm_fences(response);

    let cards: Vec<GeneratedCard> = serde_json::from_str(cleaned)
        .map_err(|e| format!("Failed to parse card generation response: {e}"))?;

    // Filter out cards with empty front/back
    let valid: Vec<GeneratedCard> = cards
        .into_iter()
        .filter(|c| {
            let ok = !c.front.trim().is_empty() && !c.back.trim().is_empty();
            if !ok {
                warn!("Skipping generated card with empty front or back");
            }
            ok
        })
        .collect();

    if valid.is_empty() {
        return Err("No valid cards generated".to_string());
    }

    Ok(valid)
}

/// Build a summary of existing cards for duplicate avoidance.
///
/// Returns None if there are no existing cards.
pub fn summarize_existing_cards(cards: &[(String, String)]) -> Option<String> {
    if cards.is_empty() {
        return None;
    }

    let summary: Vec<String> = cards
        .iter()
        .take(30) // Cap at 30 to avoid prompt overflow
        .map(|(front, back)| {
            let front_truncated = truncate_chars(front, 80, "…");
            let back_truncated = truncate_chars(back, 80, "…");
            format!("- Q: {} → A: {}", front_truncated, back_truncated)
        })
        .collect();

    Some(summary.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_cards() {
        let json = r#"[
            {"front": "What is photosynthesis?", "back": "The process by which plants convert light to energy", "card_type": "basic", "tags": ["biology"], "source_context": "Photosynthesis is the process...", "cloze_data": null, "vocab_data": null},
            {"front": "{{c1::Mitochondria}} is the powerhouse of the cell", "back": "Mitochondria is the powerhouse of the cell", "card_type": "cloze", "tags": ["biology", "cell"], "source_context": "The mitochondria...", "cloze_data": {"clozes": [{"index": 1, "hint": "organelle"}]}, "vocab_data": null}
        ]"#;
        let cards = parse_generated_cards(json).unwrap();
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].card_type, "basic");
        assert_eq!(cards[1].card_type, "cloze");
    }

    #[test]
    fn parse_with_markdown_fences() {
        let json = "```json\n[{\"front\": \"Q\", \"back\": \"A\", \"card_type\": \"basic\", \"tags\": [], \"source_context\": null, \"cloze_data\": null, \"vocab_data\": null}]\n```";
        let cards = parse_generated_cards(json).unwrap();
        assert_eq!(cards.len(), 1);
    }

    #[test]
    fn skips_empty_cards() {
        let json = r#"[
            {"front": "Good card", "back": "Good answer", "card_type": "basic", "tags": [], "source_context": null, "cloze_data": null, "vocab_data": null},
            {"front": "", "back": "No front", "card_type": "basic", "tags": [], "source_context": null, "cloze_data": null, "vocab_data": null}
        ]"#;
        let cards = parse_generated_cards(json).unwrap();
        assert_eq!(cards.len(), 1);
    }

    #[test]
    fn error_on_invalid_json() {
        let result = parse_generated_cards("not json");
        assert!(result.is_err());
    }

    #[test]
    fn error_on_all_empty() {
        let json = r#"[{"front": "", "back": "", "card_type": "basic", "tags": [], "source_context": null, "cloze_data": null, "vocab_data": null}]"#;
        let result = parse_generated_cards(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No valid cards"));
    }

    #[test]
    fn parse_vocabulary_card() {
        let json = r#"[{"front": "食べる", "back": "to eat", "card_type": "vocabulary", "tags": ["japanese", "n5"], "source_context": "食べる means to eat", "cloze_data": null, "vocab_data": {"word": "食べる", "reading": "たべる", "meaning": "to eat", "example_sentence": "寿司を食べる", "part_of_speech": "verb"}}]"#;
        let cards = parse_generated_cards(json).unwrap();
        assert_eq!(cards[0].card_type, "vocabulary");
        assert!(cards[0].vocab_data.is_some());
    }

    #[test]
    fn build_prompt_without_existing() {
        let ctx = CardGenerationContext {
            note_content: "Test content".to_string(),
            note_title: "Test Note".to_string(),
            existing_cards_summary: None,
        };
        let (system, user) = build_generation_prompt(&ctx);
        assert!(system.contains("flashcard generation"));
        assert!(user.contains("Test content"));
        assert!(!user.contains("duplicates"));
    }

    #[test]
    fn build_prompt_with_existing() {
        let ctx = CardGenerationContext {
            note_content: "Test content".to_string(),
            note_title: "Test Note".to_string(),
            existing_cards_summary: Some("- Q: What is X? → A: Y".to_string()),
        };
        let (_, user) = build_generation_prompt(&ctx);
        assert!(user.contains("duplicates"));
        assert!(user.contains("What is X?"));
    }

    #[test]
    fn summarize_existing_empty() {
        assert!(summarize_existing_cards(&[]).is_none());
    }

    #[test]
    fn summarize_existing_truncates() {
        let cards = vec![("Q1".to_string(), "A1".to_string())];
        let summary = summarize_existing_cards(&cards).unwrap();
        assert!(summary.contains("Q1"));
        assert!(summary.contains("A1"));
    }
}
