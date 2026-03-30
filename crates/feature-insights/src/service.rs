//! InsightService — orchestrates the insight generation pipeline.
//!
//! Phase 2: adds scope resolution, smart merge, prompt building, and cognitive context.
//! The LLM call itself stays in `app-core`'s insight handler (it depends on
//! `DynProvider` and event emission). This service orchestrates everything
//! before and after the LLM call.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

use crate::cross_domain::{self, EntityDomain, EntityRef, HeuristicConfig, HeuristicInput};
use crate::merge::SmartMergeEngine;
use crate::progress::ProgressComputer;
use crate::progress_repo::InsightProgressRepo;
use crate::prompt_builder::{InsightContext, PromptBuilder};
use crate::repo::InsightReviewRepo;
use crate::scope::ScopeResolver;
use crate::traits::{CrossDomainSearcher, FlashcardAccessor, InsightEmbedder};
use crate::types::{
    InsightContent, InsightReviewRow, ProgressSnapshotRow, ProgressWeights, ScopeConfig,
};
use feature_notes::models::NoteRow;

/// Result of pre-generation pipeline — ready for LLM call.
pub struct PreparedInsight {
    pub context: InsightContext,
    pub scope_note_ids: Vec<String>,
    pub parent_insight_id: Option<String>,
    pub overlap_score: f64,
}

/// The central orchestrator for insight generation and retrieval.
///
/// Event emission (streaming, tab-done) stays in `app-core`'s insight handler
/// since it depends on the transport-specific `AppEventEmitter`.
pub struct InsightService {
    pub(crate) repo: InsightReviewRepo,
    pub(crate) progress_repo: InsightProgressRepo,
    pub(crate) scope_resolver: Arc<dyn ScopeResolver>,
    pub(crate) merge_engine: SmartMergeEngine,
    pub(crate) prompt_builder: PromptBuilder,
    pub(crate) flashcards: Arc<dyn FlashcardAccessor>,
    pub(crate) embedder: Arc<dyn InsightEmbedder>,
    pub(crate) cross_domain_searcher: Arc<dyn CrossDomainSearcher>,
    pub(crate) progress_weights: ProgressWeights,
    pub(crate) domain_bus: Option<Arc<bus::DomainEventBus>>,
}

impl InsightService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        repo: InsightReviewRepo,
        progress_repo: InsightProgressRepo,
        scope_resolver: Arc<dyn ScopeResolver>,
        merge_engine: SmartMergeEngine,
        prompt_builder: PromptBuilder,
        flashcards: Arc<dyn FlashcardAccessor>,
        embedder: Arc<dyn InsightEmbedder>,
        cross_domain_searcher: Arc<dyn CrossDomainSearcher>,
        progress_weights: ProgressWeights,
        domain_bus: Option<Arc<bus::DomainEventBus>>,
    ) -> Self {
        Self {
            repo,
            progress_repo,
            scope_resolver,
            merge_engine,
            prompt_builder,
            flashcards,
            embedder,
            cross_domain_searcher,
            progress_weights,
            domain_bus,
        }
    }

    /// Check for a cross-domain connection from a newly created/updated entity.
    ///
    /// Skips entities with titles shorter than 10 characters. Uses LanceDB
    /// vector search to find semantically similar entities in other domains,
    /// then evaluates the cross-domain heuristic and publishes
    /// `DomainEvent::CrossDomainDotReady` if a connection is found.
    pub async fn check_cross_domain(
        &self,
        source_domain: EntityDomain,
        id: String,
        title: String,
        created_at: DateTime<Utc>,
    ) -> Option<cross_domain::CrossDomainDot> {
        if title.len() < 10 {
            return None;
        }

        let source = EntityRef {
            domain: source_domain.clone(),
            id,
            title: title.clone(),
        };

        // Search other domain embedding tables for semantically similar entities.
        let hits = self
            .cross_domain_searcher
            .search_other_domains(&source_domain, &source.id, &source.title)
            .await;

        let vector_hits: Vec<(EntityRef, f64, DateTime<Utc>)> = hits
            .into_iter()
            .map(|h| (h.entity, h.cosine, h.created_at))
            .collect();

        let input = HeuristicInput {
            source_has_content: title.len() >= 10,
            source: source.clone(),
            source_created: created_at,
            vector_hits,
            frequency_data: vec![],
        };

        let dot = cross_domain::evaluate_cross_domain(&input, &HeuristicConfig::default())?;

        if let Some(ref bus) = self.domain_bus {
            let event = bus::DomainEvent::CrossDomainDotReady {
                source_kind: cross_domain::domain_str(&dot.source.domain).into(),
                source_id: dot.source.id.clone(),
                source_title: dot.source.title.clone(),
                target_kind: cross_domain::domain_str(&dot.target.domain).into(),
                target_id: dot.target.id.clone(),
                target_title: dot.target.title.clone(),
                confidence: dot.confidence,
                tooltip: dot.tooltip.clone(),
                detail_route: Some(dot.detail_route.clone()),
            };
            let _ = bus.publish(event);
        }

        Some(dot)
    }

    /// Pre-generation pipeline: check merge → build context.
    ///
    /// Accepts pre-resolved `scope_note_ids` (from `resolve_scope`) to avoid
    /// double resolution. The caller resolves scope once, fetches notes, then
    /// passes both here.
    pub async fn prepare_context(
        &self,
        note: &NoteRow,
        related_notes: &[NoteRow],
        scope_note_ids: &[String],
        scope_config: &ScopeConfig,
        domains: &[String],
    ) -> Result<PreparedInsight, sqlx::Error> {
        // 1. Check for parent via smart merge
        let merge_result = self
            .merge_engine
            .find_parent(&note.id, scope_note_ids, scope_config.merge_threshold)
            .await?;

        // 2. Build full context
        let context = self
            .prompt_builder
            .build_context(
                note,
                related_notes,
                scope_config,
                domains,
                merge_result.parent.as_ref(),
            )
            .await;

        Ok(PreparedInsight {
            context,
            scope_note_ids: scope_note_ids.to_vec(),
            parent_insight_id: merge_result.parent.map(|p| p.id),
            overlap_score: merge_result.overlap_score,
        })
    }

    /// Resolve scope IDs for a note using the configured ScopeResolver.
    pub async fn resolve_scope(&self, note_id: &str, config: &ScopeConfig) -> Vec<String> {
        self.scope_resolver.resolve(note_id, config).await
    }

    /// Count cognitive context items (facts, memories) for scope preview.
    pub async fn cognitive_counts(&self, note_title: &str, note_id: &str) -> (u32, u32) {
        let facts: Vec<String> = self
            .prompt_builder
            .cognitive
            .search_facts(note_title, None, 10)
            .await;
        let memories: Vec<String> = self
            .prompt_builder
            .cognitive
            .recent_memories(note_id, 5)
            .await;
        (facts.len() as u32, memories.len() as u32)
    }

    /// Count entity connections for scope preview (deep dive).
    pub async fn entity_count(&self, note_id: &str) -> u32 {
        self.prompt_builder
            .cognitive
            .entity_neighborhood(note_id, 2)
            .await
            .len() as u32
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
    #[allow(clippy::too_many_arguments)]
    pub async fn store_insight(
        &self,
        note_id: &str,
        content: &InsightContent,
        input_hash: &str,
        scope_config: &ScopeConfig,
        persona_ids: &[String],
        parent_insight_id: Option<&str>,
        scope_note_ids: &[String],
    ) -> Result<InsightReviewRow, sqlx::Error> {
        let content_json = serde_json::to_string(content).unwrap_or_else(|_| "{}".to_string());

        // Store resolved scope IDs in scope_config.node_ids for future merge lookups
        let mut stored_scope = scope_config.clone();
        stored_scope.node_ids = scope_note_ids.to_vec();

        let row = self
            .repo
            .insert(
                note_id,
                &content_json,
                input_hash,
                &stored_scope,
                persona_ids,
                parent_insight_id,
            )
            .await?;

        // Compute initial progress snapshot (all zeros — no flashcard data yet)
        let _ = self
            .progress_repo
            .upsert(
                &row.id,
                row.version,
                0.0,
                0.0,
                0.0,
                0.0,
                &self.progress_weights,
            )
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
    pub async fn get_latest(&self, note_id: &str) -> Result<Option<InsightReviewRow>, sqlx::Error> {
        self.repo.get_latest(note_id).await
    }

    /// List all versions for a note.
    pub async fn list_versions(&self, note_id: &str) -> Result<Vec<InsightReviewRow>, sqlx::Error> {
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

            let updated_json = serde_json::to_string(&content).unwrap_or_else(|_| "{}".to_string());
            self.repo.update_content(insight_id, &updated_json).await?;
        }
        Ok(())
    }

    /// Create a ProgressComputer from the service's shared dependencies.
    fn progress_computer(&self) -> ProgressComputer {
        ProgressComputer::new(
            self.repo.clone(),
            self.progress_repo.clone(),
            Arc::clone(&self.flashcards),
            Arc::clone(&self.embedder),
            self.progress_weights.clone(),
        )
    }

    /// Compute and store a progress snapshot for an insight.
    ///
    /// Pass `quiz_score` as `Some(score)` after a user completes a quiz,
    /// or `None` for the default daily refresh path.
    pub async fn compute_progress(
        &self,
        insight_id: &str,
        note_body: &str,
        quiz_score: Option<f64>,
    ) -> Result<ProgressSnapshotRow, sqlx::Error> {
        let insight = self
            .repo
            .get(insight_id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;

        self.progress_computer()
            .compute(&insight, note_body, quiz_score)
            .await
    }

    /// Get the evolution timeline for a note — versions + progress + change notes.
    pub async fn get_evolution(
        &self,
        note_id: &str,
    ) -> Result<Vec<(InsightReviewRow, Option<ProgressSnapshotRow>)>, sqlx::Error> {
        let versions = self.repo.list_versions(note_id).await?;
        let timeline = self.progress_repo.get_timeline(note_id).await?;

        // Match versions with their progress snapshots
        let result: Vec<_> = versions
            .into_iter()
            .map(|v| {
                let snapshot = timeline
                    .iter()
                    .find(|s| s.insight_review_id == v.id)
                    .cloned();
                (v, snapshot)
            })
            .collect();

        Ok(result)
    }

    /// Get a specific insight by ID.
    pub async fn get_version(
        &self,
        insight_id: &str,
    ) -> Result<Option<InsightReviewRow>, sqlx::Error> {
        self.repo.get(insight_id).await
    }

    /// Compute input hash for cache check (same algorithm as before).
    pub fn compute_input_hash(note_title: &str, note_body: &str, related_ids: &[String]) -> String {
        let hash_input = format!("{}{}{}", note_title, note_body, related_ids.join(","));
        format!("{:x}", Sha256::digest(hash_input.as_bytes()))
    }
}
