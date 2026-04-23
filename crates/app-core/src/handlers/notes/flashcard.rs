use crate::handlers::notes::card_generation::embed_flashcard_batch;
use crate::state::AppCore;
use cognitive::repos::flashcard::ReviewQuality;
use desktop_shared::commands::{
    DeckSummaryResponse, FlashcardCreateParams, FlashcardListParams, FlashcardResponse,
    FlashcardReviewParams, FlashcardUpdateParams, NoteRetentionHealthResponse,
    StrugglingCardResponse,
};
use desktop_shared::errors::ApiError;

/// Cosine similarity between two vectors: dot(a,b) / (||a|| * ||b||).
fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    let (mut dot, mut norm_a, mut norm_b) = (0.0_f64, 0.0_f64, 0.0_f64);
    for (x, y) in a.iter().zip(b.iter()) {
        let (xf, yf) = (*x as f64, *y as f64);
        dot += xf * yf;
        norm_a += xf * xf;
        norm_b += yf * yf;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

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
    /// Fire-and-forget: embed flashcard rows in the background for semantic search.
    fn spawn_embed_if_available(&self, rows: Vec<cognitive::FlashcardRow>) {
        if let (Some(engine), Some(vs)) = (self.embedding_engine.clone(), self.vector_store.clone())
        {
            let repo_opt = self.flashcard_repo.clone();
            tokio::spawn(async move {
                embed_flashcard_batch(engine, &vs, &rows, repo_opt.as_ref()).await;
            });
        }
    }

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
                bus.publish(
                    feature_learning::LearningEvent::FlashcardReviewed {
                        atom_id: atom_id.clone(),
                        card_id: card.id.clone(),
                        quality: quality as u8,
                        recall_speed_ms: params.recall_speed_ms.unwrap_or(0) as u64,
                        new_retention_pct: retention_pct,
                        source_note_id: card.source_note_id.clone(),
                    }
                    .into(),
                );
                // NOTE: RetentionMilestoneReached is emitted by the Phase 2 decay cron
                // when retention drops below thresholds and then recovers via review.
                // Not meaningful here since retention_pct is a static 0.9 estimate.
            }
            // Update atom retention + touch last_interaction_ts in one DB call
            if let Some(atom_repo) = &self.knowledge_atom_repo {
                let _ = atom_repo
                    .update_retention(atom_id, retention_pct, card.stability, card.difficulty)
                    .await;
                // Cross-reinforce linked semantic fact (flashcard -> fact bridge)
                if let Ok(Some(atom)) = atom_repo.get(atom_id).await {
                    if let Some(ref fact_id) = atom.semantic_fact_id {
                        let fact_repo = cognitive::SemanticFactRepo::new(repo.pool().clone());
                        if let Ok(Some(fact)) = fact_repo.get(fact_id).await {
                            let new_stability =
                                cognitive::decay::update_stability(fact.stability, true, 30.0);
                            let _ = fact_repo.record_access(fact_id, new_stability).await;
                            tracing::debug!(
                                "Flashcard review reinforced fact {} (stability: {:.2} -> {:.2})",
                                fact_id,
                                fact.stability,
                                new_stability
                            );
                        }
                    }
                }
            }
        }

        // Fire-and-forget: propagate review to related cards via knowledge graph (FIRe).
        let prop_repo = repo.clone();
        let prop_card = card.clone();
        let prop_quality = params.quality.clone();
        tokio::spawn(async move {
            if let Err(e) =
                super::graph_propagation::propagate_review(&prop_repo, &prop_card, &prop_quality)
                    .await
            {
                tracing::warn!("Graph propagation failed: {e}");
            }
        });

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
            difficulty_estimate: None,
            prerequisite_concepts: None,
        };
        let row = repo
            .create_single(card)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        // Fire-and-forget: embed the newly created card in the background.
        self.spawn_embed_if_available(vec![row.clone()]);

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

        // Fire-and-forget: re-embed updated card for semantic grading accuracy.
        self.spawn_embed_if_available(vec![row.clone()]);

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

    pub async fn flashcard_list_struggling(
        &self,
        limit: i64,
    ) -> Result<Vec<StrugglingCardResponse>, ApiError> {
        let repo = self.flashcard_repo()?;
        let cards = repo
            .list_struggling_cards(limit)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(cards
            .into_iter()
            .map(|c| StrugglingCardResponse {
                id: c.id,
                front: c.front,
                back: c.back,
                deck: c.deck,
                lapses: c.lapses,
                review_count: c.review_count,
                source_note_id: c.source_note_id,
            })
            .collect())
    }

    pub async fn note_retention_health(
        &self,
        note_id: String,
    ) -> Result<Option<NoteRetentionHealthResponse>, ApiError> {
        let repo = self.flashcard_repo()?;
        let result = repo
            .note_retention_health(&note_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(result.map(|(avg_stab, total_cards, total_lapses)| {
            let lapse_penalty = (total_lapses as f64 / total_cards.max(1) as f64).min(1.0) * 0.3;
            let health = (avg_stab / 30.0).min(1.0) * (1.0 - lapse_penalty);
            NoteRetentionHealthResponse {
                avg_stability: avg_stab,
                total_cards,
                total_lapses,
                health_score: health.clamp(0.0, 1.0),
            }
        }))
    }

    /// Compute cosine similarity between a user's answer and the stored back-side
    /// embedding for a given card.
    ///
    /// Returns a score in [0.0, 1.0]. Returns 0.0 if:
    /// - No embedding engine is available
    /// - No vector store is available
    /// - The card has no stored embedding
    /// - Embedding the user answer fails
    pub async fn compute_answer_similarity(&self, card_id: &str, user_answer: &str) -> f64 {
        let (Some(engine), Some(vs)) = (self.embedding_engine.clone(), self.vector_store.clone())
        else {
            return 0.0;
        };

        // Embed the user's answer
        let text = common::truncate_at_boundary(user_answer, 2000).to_string();
        let answer_vec = match engine.embed_async(text).await {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!(card_id, "answer embedding failed: {e}");
                return 0.0;
            }
        };

        // Fetch the stored back-side embedding directly by ID
        let back_id = format!("{card_id}_back");
        let back_vec = match vs.get_embedding("flashcard_embeddings", &back_id).await {
            Ok(Some(v)) => v,
            Ok(None) => {
                tracing::debug!(card_id, "no back embedding found");
                return 0.0;
            }
            Err(e) => {
                tracing::debug!(card_id, "flashcard embedding lookup failed: {e}");
                return 0.0;
            }
        };

        // Compute cosine similarity in-process
        cosine_similarity(&answer_vec, &back_vec).clamp(0.0, 1.0)
    }
}
