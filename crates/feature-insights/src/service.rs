//! InsightService — orchestrates the insight generation pipeline.
//!
//! This is the Phase 1 version that preserves the existing 5-tab pipeline
//! but stores results in the new versioned `insight_reviews` table.
//! Smart Merge, Scope Resolution, and Learning Progress are added in Phases 2-4.

use std::sync::Arc;

use sha2::{Digest, Sha256};

use crate::progress_repo::InsightProgressRepo;
use crate::repo::InsightReviewRepo;
use crate::traits::{FlashcardAccessor, InsightEmbedder};
use crate::types::{InsightContent, InsightReviewRow, ProgressWeights, ScopeConfig};

/// The central orchestrator for insight generation and retrieval.
///
/// Note: Event emission (streaming, tab-done) stays in `app-core`'s
/// insight handler since it depends on the transport-specific `AppEventEmitter`.
/// The service handles storage and retrieval only.
pub struct InsightService {
    pub(crate) repo: InsightReviewRepo,
    pub(crate) progress_repo: InsightProgressRepo,
    /// Reserved for Phase 2+ learning progress computation.
    #[allow(dead_code)]
    pub(crate) flashcards: Arc<dyn FlashcardAccessor>,
    pub(crate) embedder: Arc<dyn InsightEmbedder>,
    pub(crate) progress_weights: ProgressWeights,
}

impl InsightService {
    pub fn new(
        repo: InsightReviewRepo,
        progress_repo: InsightProgressRepo,
        flashcards: Arc<dyn FlashcardAccessor>,
        embedder: Arc<dyn InsightEmbedder>,
        progress_weights: ProgressWeights,
    ) -> Self {
        Self {
            repo,
            progress_repo,
            flashcards,
            embedder,
            progress_weights,
        }
    }

    /// Check if a fresh insight exists for the given hash. Returns it if found.
    pub async fn check_cache(
        &self,
        note_id: &str,
        input_hash: &str,
    ) -> Result<Option<InsightReviewRow>, sqlx::Error> {
        self.repo.get_by_hash(note_id, input_hash).await
    }

    /// Store a completed insight (called after the background pipeline finishes).
    pub async fn store_insight(
        &self,
        note_id: &str,
        content: &InsightContent,
        input_hash: &str,
        scope_config: &ScopeConfig,
        persona_ids: &[String],
    ) -> Result<InsightReviewRow, sqlx::Error> {
        let content_json =
            serde_json::to_string(content).unwrap_or_else(|_| "{}".to_string());

        let row = self
            .repo
            .insert(note_id, &content_json, input_hash, scope_config, persona_ids, None)
            .await?;

        // Compute initial progress snapshot (all zeros — no flashcard data yet)
        let _ = self
            .progress_repo
            .upsert(&row.id, row.version, 0.0, 0.0, 0.0, 0.0, &self.progress_weights)
            .await;

        // Embed the content for future semantic drift + dedup (best effort)
        let embed_id = row.id.clone();
        let embed_content = content_json.clone();
        let embedder = Arc::clone(&self.embedder);
        tokio::spawn(async move {
            let _ = embedder.embed_and_store(&embed_id, &embed_content).await;
        });

        Ok(row)
    }

    /// Get the latest insight for a note (no LLM call).
    pub async fn get_latest(
        &self,
        note_id: &str,
    ) -> Result<Option<InsightReviewRow>, sqlx::Error> {
        self.repo.get_latest(note_id).await
    }

    /// List all versions for a note.
    pub async fn list_versions(
        &self,
        note_id: &str,
    ) -> Result<Vec<InsightReviewRow>, sqlx::Error> {
        self.repo.list_versions(note_id).await
    }

    /// Update a single tab in the latest insight's content.
    pub async fn update_tab(
        &self,
        insight_id: &str,
        tab_name: &str,
        tab_content: &str,
    ) -> Result<(), sqlx::Error> {
        let row = self.repo.get(insight_id).await?;
        if let Some(row) = row {
            let mut content: InsightContent =
                serde_json::from_str(&row.content).unwrap_or_default();

            match tab_name {
                "synthesis" => content.synthesis = Some(tab_content.to_string()),
                "gaps" | "gap_analysis" => content.gap_analysis = Some(tab_content.to_string()),
                "assessment" | "self_assessment" => {
                    content.self_assessment = Some(tab_content.to_string())
                }
                "concept-map" | "concept_map" => {
                    content.concept_map = Some(tab_content.to_string())
                }
                "perspectives" => content.perspectives = Some(tab_content.to_string()),
                _ => return Ok(()),
            }

            let updated_json =
                serde_json::to_string(&content).unwrap_or_else(|_| "{}".to_string());
            self.repo.update_content(insight_id, &updated_json).await?;
        }
        Ok(())
    }

    /// Compute input hash for cache check (same algorithm as before).
    pub fn compute_input_hash(note_title: &str, note_body: &str, related_ids: &[String]) -> String {
        let hash_input = format!("{}{}{}", note_title, note_body, related_ids.join(","));
        format!("{:x}", Sha256::digest(hash_input.as_bytes()))
    }
}
