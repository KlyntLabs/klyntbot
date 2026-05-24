use std::sync::Arc;

/// Map a difficulty estimate (1–5) to initial FSRS-5 `(stability, difficulty)` parameters.
///
/// Higher difficulty estimates produce lower initial stability and higher FSRS difficulty,
/// which causes the scheduler to review the card more aggressively at the start.
fn difficulty_to_fsrs(estimate: Option<i32>) -> (f64, f64) {
    match estimate.unwrap_or(3) {
        1 => (4.0, 3.0), // Easy recall: high stability, low difficulty
        2 => (3.0, 4.0),
        3 => (2.0, 5.0), // Default
        4 => (1.2, 6.5),
        _ => (0.8, 8.0), // Hard synthesis: low stability, high difficulty
    }
}

use desktop_shared::commands::{
    FlashcardGenerateParams, FlashcardGenerateResponse, FlashcardResponse,
    FlashcardSaveGeneratedParams, GeneratedCardPreview,
};
use desktop_shared::errors::ApiError;
use storage::VectorStore;
use tools::embedding_engine::EmbeddingEngine;

use crate::state::AppCore;

/// Embed both front and back of a batch of flashcard rows into `flashcard_embeddings`.
///
/// This is fire-and-forget: errors are logged but not propagated.
/// Each card produces two embedding records with composite IDs:
///   - `"{card_id}_front"` (side = "front")
///   - `"{card_id}_back"`  (side = "back")
///
/// When `pool` is provided, `back_embedding_updated_at` is stamped after a successful
/// back embedding upsert.
/// Embed one side (front or back) of a flashcard into `flashcard_embeddings`.
///
/// Returns `true` if the embedding was successfully stored.
async fn embed_one_side(
    engine: &Arc<EmbeddingEngine>,
    vector_store: &VectorStore,
    card_id: &str,
    text: &str,
    side: &str,
) -> bool {
    let truncated = common::truncate_at_boundary(text, 2000).to_string();
    match engine.clone().embed_async(truncated).await {
        Ok(vec) => {
            let id = format!("{card_id}_{side}");
            let extras: &[(&str, &str)] = &[("card_id", card_id), ("side", side)];
            if let Err(e) = vector_store
                .upsert_embedding("flashcard_embeddings", &id, &vec, extras)
                .await
            {
                tracing::warn!(card_id, side, "flashcard embedding upsert failed: {e}");
                return false;
            }
            true
        }
        Err(e) => {
            tracing::warn!(card_id, side, "flashcard embedding failed: {e}");
            false
        }
    }
}

pub(crate) async fn embed_flashcard_batch(
    engine: Arc<EmbeddingEngine>,
    vector_store: &VectorStore,
    cards: &[cognitive::FlashcardRow],
    repo: Option<&cognitive::FlashcardRepo>,
) {
    for card in cards {
        let card_id = card.id.as_str();

        embed_one_side(&engine, vector_store, card_id, &card.front, "front").await;

        if embed_one_side(&engine, vector_store, card_id, &card.back, "back").await {
            if let Some(r) = repo {
                let _ = r.update_embedding_timestamp(card_id).await;
            }
        }
    }
}

impl AppCore {
    /// Generate flashcard previews from a note or raw text.
    ///
    /// Calls the LLM with the note content + existing cards context.
    /// Returns preview cards for the user to approve/edit before saving.
    #[tracing::instrument(skip(self), err)]
    pub async fn flashcard_generate(
        &self,
        params: FlashcardGenerateParams,
    ) -> Result<FlashcardGenerateResponse, ApiError> {
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
        let (provider, chat_params) = self.cognitive_chat_context(4096).await?;

        let messages = vec![
            providers::Message::System {
                content: system_prompt,
            },
            providers::Message::User {
                content: providers::UserContent::Text(user_prompt),
            },
        ];

        let response = provider
            .chat(&messages, None, &chat_params, &[])
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
                difficulty_estimate: g.difficulty_estimate,
                prerequisite_concepts: g.prerequisite_concepts,
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
    #[tracing::instrument(skip(self), err)]
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
                let (stability, difficulty) = difficulty_to_fsrs(c.difficulty_estimate);
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
                    stability,
                    difficulty,
                    difficulty_estimate: c.difficulty_estimate,
                    prerequisite_concepts: c
                        .prerequisite_concepts
                        .as_ref()
                        .map(|v| serde_json::to_string(v).unwrap_or_default()),
                }
            })
            .collect();

        let rows = repo
            .create_batch(cards)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        // Link flashcards to existing knowledge atoms by subject match
        if let Ok(atom_repo) = self.knowledge_atom_repo() {
            let atoms = if let Some(nid) = &params.note_id {
                atom_repo.list_for_note(nid).await.unwrap_or_default()
            } else {
                vec![]
            };
            let active_atoms: Vec<_> = atoms.into_iter().filter(|a| a.status == "active").collect();

            for row in &rows {
                let front_lower = row.front.to_lowercase();
                if let Some(atom) = active_atoms
                    .iter()
                    .find(|a| front_lower.contains(&a.subject.to_lowercase()))
                {
                    let _ = repo.update_atom_id(&row.id, &atom.id).await;
                }
            }
        }

        // Fire-and-forget: embed all saved cards in the background.
        if let (Some(engine), Some(vs)) = (self.embedding_engine(), self.vector_store())
        {
            let rows_for_embed = rows.clone();
            let repo_opt = self.flashcard_repo().ok();
            tokio::spawn(async move {
                embed_flashcard_batch(engine, &vs, &rows_for_embed, repo_opt.as_ref()).await;
            });
        }

        Ok(rows
            .into_iter()
            .map(super::flashcard::flashcard_to_response)
            .collect())
    }
}
