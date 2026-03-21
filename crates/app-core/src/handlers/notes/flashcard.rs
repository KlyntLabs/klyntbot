use crate::state::AppCore;
use cognitive::repos::flashcard::ReviewQuality;
use desktop_shared::commands::{
    DeckSummaryResponse, FlashcardCreateParams, FlashcardListParams, FlashcardResponse,
    FlashcardReviewParams, FlashcardUpdateParams,
};
use desktop_shared::errors::ApiError;

fn parse_json_col(s: Option<&str>) -> Option<serde_json::Value> {
    s.and_then(|v| serde_json::from_str(v).ok())
}

/// Map a FlashcardRow to a FlashcardResponse.
pub(crate) fn flashcard_to_response(r: cognitive::FlashcardRow) -> FlashcardResponse {
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
        let repo = self.flashcard_repo()?;
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
        let repo = self.flashcard_repo()?;
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
        let repo = self.flashcard_repo()?;
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

        // Emit AtomFlashcardReviewed if card is linked to an atom
        if let Some(atom_id) = &card.atom_id {
            // FSRS-5: R(t) = (1 + t/(9*S))^(-1). Just-reviewed card → t≈0 → R≈1.0.
            // Use 0.9 (desired retention) as a conservative estimate for the stored metric.
            let retention_pct = 0.9_f64;

            if let Some(bus) = &self.domain_event_bus {
                let _ = bus.publish(bus::DomainEvent::AtomFlashcardReviewed {
                    atom_id: atom_id.clone(),
                    card_id: card.id.clone(),
                    quality: quality as u8,
                    recall_speed_ms: params.recall_speed_ms.unwrap_or(0) as u64,
                    new_retention_pct: retention_pct,
                    source_note_id: card.source_note_id.clone(),
                });
            }
            // Update atom retention + touch last_interaction_ts in one DB call
            if let Some(atom_repo) = &self.knowledge_atom_repo {
                let _ = atom_repo
                    .update_retention(atom_id, retention_pct, card.stability, card.difficulty)
                    .await;
            }
        }

        Ok(flashcard_to_response(card))
    }

    pub async fn flashcard_get(&self, id: &str) -> Result<FlashcardResponse, ApiError> {
        let repo = self.flashcard_repo()?;
        let card = repo
            .get_by_id(id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Card not found"))?;
        Ok(flashcard_to_response(card))
    }

    pub async fn flashcard_create(
        &self,
        params: FlashcardCreateParams,
    ) -> Result<FlashcardResponse, ApiError> {
        let repo = self.flashcard_repo()?;
        let card = cognitive::NewFlashcard {
            source_note_id: params.source_note_id,
            source_context: None,
            atom_id: None,
            deck: params.deck,
            front: params.front,
            back: params.back,
            card_type: cognitive::CardType::parse(&params.card_type),
            cloze_data: params.cloze_data,
            vocab_data: params.vocab_data,
            image_data: None,
            tags: params.tags.unwrap_or_default(),
            stability: 1.0,
            difficulty: 5.0,
        };
        let row = repo
            .create_single(card)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(flashcard_to_response(row))
    }

    pub async fn flashcard_update(
        &self,
        params: FlashcardUpdateParams,
    ) -> Result<FlashcardResponse, ApiError> {
        let repo = self.flashcard_repo()?;
        let row = repo
            .update_card(
                &params.id,
                &params.front,
                &params.back,
                &params.deck,
                &params.tags.unwrap_or_default(),
                params.cloze_data.as_ref(),
                params.vocab_data.as_ref(),
            )
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(flashcard_to_response(row))
    }

    pub async fn flashcard_list_cards(
        &self,
        params: FlashcardListParams,
    ) -> Result<Vec<FlashcardResponse>, ApiError> {
        let repo = self.flashcard_repo()?;
        let cards = repo
            .list_all_in_deck(
                &params.deck,
                params.limit.unwrap_or(100),
                params.offset.unwrap_or(0),
            )
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(cards.into_iter().map(flashcard_to_response).collect())
    }

    pub async fn flashcard_delete(&self, id: &str) -> Result<bool, ApiError> {
        let repo = self.flashcard_repo()?;
        repo.delete_card(id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))
    }

    pub async fn flashcard_get_all_due(
        &self,
        limit: i64,
    ) -> Result<Vec<FlashcardResponse>, ApiError> {
        let repo = self.flashcard_repo()?;
        let cards = repo
            .get_all_due_cards(limit)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(cards.into_iter().map(flashcard_to_response).collect())
    }

    pub async fn flashcard_total_due(&self) -> Result<i64, ApiError> {
        let repo = self.flashcard_repo()?;
        repo.total_due_count()
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))
    }
}
