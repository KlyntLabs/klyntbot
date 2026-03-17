use crate::state::AppCore;
use cognitive::repos::flashcard::ReviewQuality;
use desktop_shared::commands::{DeckSummaryResponse, FlashcardReviewParams, FlashcardResponse};
use desktop_shared::errors::ApiError;

/// Map a FlashcardRow to a FlashcardResponse.
pub(super) fn flashcard_to_response(r: cognitive::FlashcardRow) -> FlashcardResponse {
    FlashcardResponse {
        id: r.id,
        deck: r.deck,
        question: r.question,
        answer: r.answer,
        card_type: r.card_type,
        choices: r
            .choices
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        stability: r.stability,
        difficulty: r.difficulty,
        due_at: r.due_at,
        state: r.state,
        review_count: r.review_count,
        created_at: r.created_at,
    }
}

impl AppCore {
    pub async fn flashcard_list_decks(&self) -> Result<Vec<DeckSummaryResponse>, ApiError> {
        let repo = self
            .flashcard_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Flashcard repo not available"))?;
        let decks = repo
            .list_decks()
            .await
            .map_err(|e: sqlx::Error| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(decks
            .into_iter()
            .map(|d| DeckSummaryResponse {
                name: d.name,
                card_count: d.card_count,
                due_count: d.due_count,
            })
            .collect())
    }

    pub async fn flashcard_get_due(
        &self,
        deck: &str,
        limit: i64,
    ) -> Result<Vec<FlashcardResponse>, ApiError> {
        let repo = self
            .flashcard_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Flashcard repo not available"))?;
        let cards = repo
            .get_due_cards(deck, limit)
            .await
            .map_err(|e: sqlx::Error| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(cards.into_iter().map(flashcard_to_response).collect())
    }

    pub async fn flashcard_record_review(
        &self,
        params: FlashcardReviewParams,
    ) -> Result<FlashcardResponse, ApiError> {
        let repo = self
            .flashcard_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Flashcard repo not available"))?;
        let quality = match params.quality.as_str() {
            "again" => ReviewQuality::Again,
            "hard" => ReviewQuality::Hard,
            "good" => ReviewQuality::Good,
            "easy" => ReviewQuality::Easy,
            _ => {
                return Err(ApiError::new(
                    "VALIDATION",
                    "Invalid review quality: must be again|hard|good|easy",
                ))
            }
        };
        let card = repo
            .record_review(&params.card_id, quality)
            .await
            .map_err(|e: sqlx::Error| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(flashcard_to_response(card))
    }
}
