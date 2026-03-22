use desktop_shared::commands::{
    FlashcardGenerateParams, FlashcardGenerateResponse, FlashcardResponse,
    FlashcardSaveGeneratedParams, GeneratedCardPreview,
};
use desktop_shared::errors::ApiError;

use crate::state::AppCore;

impl AppCore {
    /// Generate flashcard previews from a note or raw text.
    ///
    /// Calls the LLM with the note content + existing cards context.
    /// Returns preview cards for the user to approve/edit before saving.
    pub async fn flashcard_generate(
        &self,
        params: FlashcardGenerateParams,
    ) -> Result<FlashcardGenerateResponse, ApiError> {
        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;

        // Resolve content: from note or raw text
        let (note_title, note_content, note_id) = match (&params.note_id, &params.text_content) {
            (Some(nid), _) => {
                let note = self
                    .note_repo
                    .get_note(nid)
                    .await
                    .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
                    .ok_or_else(|| ApiError::new("NOT_FOUND", "Note not found"))?;
                (note.title.clone(), note.body.clone(), Some(nid.clone()))
            }
            (_, Some(text)) => {
                if text.trim().is_empty() {
                    return Err(ApiError::new("INVALID_INPUT", "Text content is empty"));
                }
                ("Pasted Text".to_string(), text.clone(), None)
            }
            _ => {
                return Err(ApiError::new(
                    "INVALID_INPUT",
                    "Either note_id or text_content is required",
                ));
            }
        };

        // Fetch existing cards for duplicate avoidance
        let existing_summary = if let Some(ref nid) = note_id {
            let repo = self.flashcard_repo()?;
            let existing = repo.list_by_note(nid).await.unwrap_or_default();
            let pairs: Vec<(String, String)> = existing
                .iter()
                .map(|c| (c.front.clone(), c.back.clone()))
                .collect();
            feature_learning::summarize_existing_cards(&pairs)
        } else {
            None
        };

        // Build prompt
        let ctx = feature_learning::CardGenerationContext {
            note_content,
            note_title: note_title.clone(),
            existing_cards_summary: existing_summary,
        };
        let (system_prompt, user_prompt) = feature_learning::build_generation_prompt(&ctx);

        // Call LLM
        let config = self.config.read().await;
        let chat_params = providers::cognitive_chat_params(&config, 4096);
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
            .map_err(|e| ApiError::new("LLM_ERROR", format!("Card generation failed: {e}")))?;

        let response_text = response
            .content
            .ok_or_else(|| ApiError::new("LLM_ERROR", "Empty response from LLM"))?;

        // Parse response
        let generated = feature_learning::parse_generated_cards(&response_text)
            .map_err(|e| ApiError::new("PARSE_ERROR", e))?;

        // Convert to preview type
        let cards: Vec<GeneratedCardPreview> = generated
            .into_iter()
            .map(|g| GeneratedCardPreview {
                front: g.front,
                back: g.back,
                card_type: g.card_type,
                tags: g.tags,
                source_context: g.source_context,
                cloze_data: g.cloze_data,
                vocab_data: g.vocab_data,
            })
            .collect();

        // Suggest deck name from note title or hint
        let deck_suggestion = params
            .deck_hint
            .unwrap_or_else(|| note_title.chars().take(40).collect());

        Ok(FlashcardGenerateResponse {
            cards,
            deck_suggestion,
        })
    }

    /// Save user-approved generated cards as real flashcards.
    pub async fn flashcard_save_generated(
        &self,
        params: FlashcardSaveGeneratedParams,
    ) -> Result<Vec<FlashcardResponse>, ApiError> {
        let repo = self.flashcard_repo()?;

        let cards: Vec<cognitive::NewFlashcard> = params
            .cards
            .iter()
            .map(|c| {
                let card_type = cognitive::CardType::parse(&c.card_type);
                cognitive::NewFlashcard {
                    source_note_id: params.note_id.clone(),
                    source_context: c.source_context.clone(),
                    atom_id: None,
                    deck: params.deck.clone(),
                    front: c.front.clone(),
                    back: c.back.clone(),
                    card_type,
                    cloze_data: c.cloze_data.clone(),
                    vocab_data: c.vocab_data.clone(),
                    image_data: None,
                    tags: c.tags.clone(),
                    stability: 1.0,
                    difficulty: 5.0,
                    difficulty_estimate: None,
                    prerequisite_concepts: None,
                }
            })
            .collect();

        let rows = repo
            .create_batch(cards)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(super::flashcard::flashcard_to_response)
            .collect())
    }
}
