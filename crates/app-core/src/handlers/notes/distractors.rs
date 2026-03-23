use desktop_shared::commands::{FlashcardDistractorParams, FlashcardDistractorResponse};
use desktop_shared::errors::ApiError;

use crate::state::AppCore;

/// Parsed response from the distractor-generation LLM call.
#[derive(serde::Deserialize)]
struct LlmDistractorResult {
    distractors: Vec<String>,
}

impl AppCore {
    /// Generate plausible but incorrect multiple-choice distractors for a flashcard.
    ///
    /// Results are cached in `card_distractors` on the first call; subsequent calls for
    /// the same card return immediately with `cached: true`.
    pub async fn flashcard_generate_distractors(
        &self,
        params: FlashcardDistractorParams,
    ) -> Result<FlashcardDistractorResponse, ApiError> {
        let repo = self.flashcard_repo()?;

        // 1. Fetch the card.
        let card = repo
            .get_by_id(&params.card_id)
            .await
            .map_err(|e| ApiError::new("STORAGE_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Flashcard not found"))?;

        // 2. Return cached distractors if available.
        if let Some(cached_json) = &card.card_distractors {
            if !cached_json.is_empty() {
                if let Ok(parsed) = serde_json::from_str::<LlmDistractorResult>(cached_json) {
                    if !parsed.distractors.is_empty() {
                        return Ok(FlashcardDistractorResponse {
                            distractors: parsed.distractors,
                            cached: true,
                        });
                    }
                }
            }
        }

        // 3. Build prompts.
        let count = params.count;
        let system_prompt = format!(
            r#"You are an expert educator creating multiple-choice distractors for flashcards.

Generate exactly {count} plausible but clearly incorrect answers for the given flashcard.

Rules:
- Each distractor must be wrong but believable to someone who partially understands the topic
- Distractors should be similar in style, length, and format to the correct answer
- Do NOT include the correct answer
- Do NOT number the distractors

Return ONLY this exact JSON (no extra text):
{{
  "distractors": ["distractor1", "distractor2", "distractor3"]
}}"#
        );

        let mut user_prompt = format!(
            "Flashcard question (front):\n{}\n\nCorrect answer (back):\n{}",
            card.front, card.back
        );
        if let Some(ctx) = &card.source_context {
            user_prompt.push_str(&format!("\n\nSource context:\n{ctx}"));
        }

        // 4. Call LLM.
        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;

        let config = self.config.read().await;
        let chat_params = providers::cognitive_chat_params(&config, 2048);
        drop(config);

        let messages = vec![
            providers::Message::System {
                content: system_prompt,
            },
            providers::Message::User {
                content: providers::UserContent::Text(user_prompt),
            },
        ];

        let response = provider
            .chat(&messages, None, &chat_params)
            .await
            .map_err(|e| ApiError::new("LLM_ERROR", e.to_string()))?;

        let text = response
            .content
            .ok_or_else(|| ApiError::new("LLM_ERROR", "Empty response from LLM"))?;

        let cleaned = common::helpers::strip_llm_fences(&text);
        let result: LlmDistractorResult = serde_json::from_str(cleaned).map_err(|e| {
            ApiError::new(
                "PARSE_ERROR",
                format!("Failed to parse distractor response: {e}"),
            )
        })?;

        // 5. Cache in DB.
        let distractors_json =
            serde_json::json!({ "distractors": &result.distractors }).to_string();
        repo.update_distractors(&params.card_id, &distractors_json)
            .await
            .map_err(|e| ApiError::new("STORAGE_ERROR", e.to_string()))?;

        Ok(FlashcardDistractorResponse {
            distractors: result.distractors,
            cached: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distractor_params_default_count() {
        let json = r#"{"cardId": "abc123"}"#;
        let params: FlashcardDistractorParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.card_id, "abc123");
        assert_eq!(params.count, 3);
    }

    #[test]
    fn test_distractor_params_explicit_count() {
        let json = r#"{"cardId": "abc123", "count": 5}"#;
        let params: FlashcardDistractorParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.count, 5);
    }

    #[test]
    fn test_distractor_response_serialization() {
        let resp = FlashcardDistractorResponse {
            distractors: vec!["wrong1".to_string(), "wrong2".to_string()],
            cached: true,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("wrong1"));
        assert!(json.contains("\"cached\":true"));
    }

    #[test]
    fn test_llm_distractor_result_parse() {
        let raw = r#"{"distractors": ["Rust", "Python", "Go"]}"#;
        let parsed: LlmDistractorResult = serde_json::from_str(raw).unwrap();
        assert_eq!(parsed.distractors.len(), 3);
        assert_eq!(parsed.distractors[0], "Rust");
    }

    #[test]
    fn test_llm_distractor_result_parse_with_fences() {
        // strip_llm_fences is tested separately; verify we handle clean JSON here
        let raw = r#"{"distractors": ["A", "B", "C"]}"#;
        let parsed: LlmDistractorResult = serde_json::from_str(raw).unwrap();
        assert_eq!(parsed.distractors, vec!["A", "B", "C"]);
    }
}
