use crate::state::AppCore;
use cognitive::repos::flashcard::ReviewQuality;
use desktop_shared::commands::{DeckSummaryResponse, FlashcardResponse, FlashcardReviewParams};
use desktop_shared::errors::ApiError;

fn parse_json_col(s: Option<&str>) -> Option<serde_json::Value> {
    s.and_then(|v| serde_json::from_str(v).ok())
}

/// Map a FlashcardRow to a FlashcardResponse.
pub(super) fn flashcard_to_response(r: cognitive::FlashcardRow) -> FlashcardResponse {
    FlashcardResponse {
        id: r.id,
        deck: r.deck,
        front: r.front,
        back: r.back,
        card_type: r.card_type,
        cloze_data: parse_json_col(r.cloze_data.as_deref()),
        vocab_data: parse_json_col(r.vocab_data.as_deref()),
        image_data: parse_json_col(r.image_data.as_deref()),
        tags: serde_json::from_str(&r.tags).unwrap_or(serde_json::Value::Array(vec![])),
        source_note_id: r.source_note_id,
        source_context: r.source_context,
        stability: r.stability,
        difficulty: r.difficulty,
        due_at: r.due_at,
        state: r.state,
        review_count: r.review_count,
        recall_speed_ms: r.recall_speed_ms,
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
            .record_review(&params.card_id, quality, params.recall_speed_ms)
            .await
            .map_err(|e: sqlx::Error| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(flashcard_to_response(card))
    }
}
