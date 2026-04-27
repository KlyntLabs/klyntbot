//! Fractional Implicit Repetition (FIRe) — knowledge graph propagation.
//!
//! When a flashcard is reviewed, related cards receive fractional stability
//! boosts (or penalties) via the knowledge graph. This strengthens connected
//! knowledge without requiring explicit review of every related card.

use cognitive::repos::flashcard::{FlashcardRepo, FlashcardRow};
use desktop_shared::errors::ApiError;

use crate::handlers::notes::flashcard::flashcard_to_response;
use crate::state::AppCore;
use desktop_shared::commands::FlashcardResponse;

/// Run FIRe propagation after a review. Returns the count of cards boosted/penalized.
///
/// Standalone function so it can be called from a spawned background task
/// (AppCore is not Clone, but FlashcardRepo is).
///
/// - quality_factor: easy=1.0, good=0.8, hard=0.3, again=0.0
/// - For "again": no positive propagation, but apply negative propagation
/// - Walk note_links graph: boost = 1.0 * quality_factor * 0.15
/// - Walk same-domain atoms: boost = 0.5 * quality_factor * 0.15
#[tracing::instrument(skip(repo, card), err)]
pub async fn propagate_review(
    repo: &FlashcardRepo,
    card: &FlashcardRow,
    quality: &str,
) -> Result<usize, String> {
    let quality_factor = match quality {
        "easy" => 1.0_f64,
        "good" => 0.8,
        "hard" => 0.3,
        "again" => 0.0,
        _ => return Ok(0),
    };

    let mut affected = 0_usize;
    let is_again = quality == "again";

    // 1. Cards linked via note_links
    if let Some(note_id) = &card.source_note_id {
        let linked = repo
            .find_cards_linked_by_notes(note_id, &card.id)
            .await
            .unwrap_or_default();

        for related in &linked {
            if is_again {
                // Negative propagation: pull related cards' due dates forward
                let penalty = 0.05; // conservative penalty via graph
                if repo
                    .apply_propagation_penalty(&related.id, penalty)
                    .await
                    .is_ok()
                {
                    affected += 1;
                }
            } else {
                let boost = 1.0 * quality_factor * 0.15;
                if repo
                    .apply_propagation_boost(&related.id, boost)
                    .await
                    .is_ok()
                {
                    affected += 1;
                }
            }
        }
    }

    // 2. Cards sharing the same atom domain
    if let Some(atom_id) = &card.atom_id {
        let domain_cards = repo
            .find_cards_same_domain(atom_id, &card.id)
            .await
            .unwrap_or_default();

        for related in &domain_cards {
            if is_again {
                let penalty = 0.03; // smaller penalty for domain-level
                if repo
                    .apply_propagation_penalty(&related.id, penalty)
                    .await
                    .is_ok()
                {
                    affected += 1;
                }
            } else {
                let boost = 0.5 * quality_factor * 0.15;
                if repo
                    .apply_propagation_boost(&related.id, boost)
                    .await
                    .is_ok()
                {
                    affected += 1;
                }
            }
        }
    }

    Ok(affected)
}

impl AppCore {
    /// Return prerequisite cards for a given card.
    ///
    /// Finds cards linked via `note_links` to this card's source note that are
    /// due within 7 days. Returns up to 3 prerequisite cards.
    #[tracing::instrument(skip(self), err)]
    pub async fn flashcard_get_prerequisites_impl(
        &self,
        card_id: &str,
    ) -> Result<Vec<FlashcardResponse>, ApiError> {
        let repo = self.flashcard_repo()?;
        let card = repo
            .get_by_id(card_id)
            .await
            .map_err(|e| ApiError::new("STORAGE_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Flashcard not found"))?;

        let Some(note_id) = &card.source_note_id else {
            return Ok(vec![]);
        };

        let linked = repo
            .find_cards_linked_by_notes(note_id, card_id)
            .await
            .unwrap_or_default();

        // Filter to cards due within 7 days, take up to 3
        let cutoff = jiff::Timestamp::now()
            .checked_add(jiff::SignedDuration::from_secs(7 * 86400))
            .unwrap_or_else(|_| jiff::Timestamp::now())
            .to_string();

        #[allow(clippy::unnecessary_map_or)] // is_none_or requires Rust 1.82, MSRV is 1.75
        let prerequisites: Vec<FlashcardResponse> = linked
            .into_iter()
            .filter(|c| c.due_at.as_deref().map_or(true, |d| d <= cutoff.as_str()))
            .take(3)
            .map(flashcard_to_response)
            .collect();

        Ok(prerequisites)
    }
}
