use desktop_shared::commands::*;
use desktop_shared::errors::ApiError;
use sha2::{Digest, Sha256};

use crate::state::AppCore;

impl AppCore {
    /// Start insight review: check cache, return initial response.
    pub async fn note_insight_review(
        &self,
        note_id: &str,
    ) -> Result<InsightReviewStarted, ApiError> {
        let note = self
            .note_repo
            .get_note(note_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Note not found"))?;

        if note.body.trim().is_empty() {
            return Err(ApiError::new("VALIDATION", "Note has no content"));
        }

        // Compute content hash: SHA-256(title + body + sorted related note IDs)
        let related_ids = self.get_related_note_ids(note_id).await;
        let hash_input = format!("{}{}{}", note.title, note.body, related_ids.join(","));
        let content_hash = format!("{:x}", Sha256::digest(hash_input.as_bytes()));

        // Check cache
        if let Some(ref repo) = self.insight_cache_repo {
            if let Ok(Some(_cached)) = repo.get_if_fresh(note_id, &content_hash).await {
                return Ok(InsightReviewStarted {
                    insight_review_id: format!(
                        "ir-{}",
                        uuid::Uuid::new_v4()
                            .to_string()
                            .split('-')
                            .next()
                            .unwrap_or("0000")
                    ),
                    content_hash,
                    cached: true,
                });
            }
        }

        let insight_review_id = format!(
            "ir-{}",
            uuid::Uuid::new_v4()
                .to_string()
                .split('-')
                .next()
                .unwrap_or("0000")
        );

        // TODO: Spawn background task for LLM calls + streaming events
        // For now, return uncached response — LLM integration in a follow-up task

        Ok(InsightReviewStarted {
            insight_review_id,
            content_hash,
            cached: false,
        })
    }

    /// Get cached insight review for instant re-open.
    pub async fn note_insight_cache_get(
        &self,
        note_id: &str,
    ) -> Result<Option<InsightReviewResponse>, ApiError> {
        let repo = match &self.insight_cache_repo {
            Some(r) => r,
            None => return Ok(None),
        };

        let cached = match repo
            .get(note_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
        {
            Some(c) => c,
            None => return Ok(None),
        };

        let self_assessment: Option<Vec<QuizQuestion>> = cached
            .self_assessment
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok());

        Ok(Some(InsightReviewResponse {
            insight_review_id: cached.id,
            note_id: cached.note_id,
            synthesis: cached.synthesis,
            gap_analysis: cached.gap_analysis,
            self_assessment,
            concept_map: cached.concept_map,
        }))
    }

    /// Save quiz questions as flashcards with FSRS init.
    pub async fn insight_save_flashcards(
        &self,
        params: InsightSaveFlashcardsParams,
    ) -> Result<Vec<FlashcardResponse>, ApiError> {
        let repo = self
            .flashcard_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Flashcard repo not available"))?;

        let cards: Vec<cognitive::NewFlashcard> = params
            .questions
            .iter()
            .map(|q| {
                let (stability, difficulty) = match q.difficulty.as_str() {
                    "easy" => (4.0, 0.3),
                    "hard" => (0.8, 0.7),
                    _ => (2.0, 0.5), // medium
                };
                cognitive::NewFlashcard {
                    source_note_id: Some(params.note_id.clone()),
                    insight_review_id: Some(params.insight_review_id.clone()),
                    deck: params.deck_name.clone(),
                    question: q.question.clone(),
                    answer: q.correct_answer.clone(),
                    card_type: if q.question_type == "multiple_choice" {
                        cognitive::CardType::MultipleChoice
                    } else {
                        cognitive::CardType::ShortAnswer
                    },
                    choices: q.choices.as_ref().map(|c| serde_json::json!(c)),
                    stability,
                    difficulty,
                }
            })
            .collect();

        let rows = repo
            .create_batch(cards)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|r| FlashcardResponse {
                id: r.id,
                deck: r.deck,
                question: r.question,
                answer: r.answer,
                card_type: r.card_type,
                choices: r.choices.as_deref().and_then(|s| serde_json::from_str(s).ok()),
                stability: r.stability,
                difficulty: r.difficulty,
                due_at: r.due_at,
                state: r.state,
                review_count: r.review_count,
                created_at: r.created_at,
            })
            .collect())
    }

    /// Regenerate a single tab.
    pub async fn note_insight_regenerate_tab(
        &self,
        _note_id: &str,
        tab: &str,
    ) -> Result<TabContent, ApiError> {
        // TODO: Re-run single LLM call, update cache
        Ok(TabContent {
            tab: tab.to_string(),
            content: String::new(),
        })
    }

    /// Helper: get sorted related note IDs for cache hash computation.
    async fn get_related_note_ids(&self, note_id: &str) -> Vec<String> {
        let backlinks = self
            .note_repo
            .get_backlinks_with_context(note_id)
            .await
            .unwrap_or_default();
        let mut ids: Vec<String> = backlinks.into_iter().map(|(note, _ctx)| note.id).collect();
        ids.sort();
        ids
    }
}
