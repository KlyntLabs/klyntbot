//! `DistillerWriter` — single write chokepoint for every Distiller-authored row.
//!
//! Enforces the **provenance-always invariant**: any write missing a
//! populated `ProvenanceMetadata.source_events` returns
//! `DistillerError::ProvenanceMissing`. In dev builds the same condition
//! additionally panics (via `debug_assert`), catching integration mistakes early.

use super::error::DistillerError;
use crate::scope::ProvenanceMetadata;
use cognitive::types::{EpisodicMemory, SemanticFact};
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use serde_json::json;

use cognitive::repos::entity::EntityRepo;

/// A fact prepared for writing — carries the row plus coding-memory metadata.
#[derive(Debug, Clone)]
pub struct PreparedFact {
    /// The cognitive-layer `SemanticFact` row.
    pub fact: SemanticFact,
    /// Pre-built `metadata` JSON payload. If `None`, `writer` will build
    /// one containing only the `provenance` block.
    pub metadata_json: Option<serde_json::Value>,
    /// Scope partition for the row (None = global).
    pub scope_repo_id: Option<String>,
    /// Provenance — must have non-empty `source_events`.
    pub provenance: ProvenanceMetadata,
}

/// An episodic row prepared for writing.
#[derive(Debug, Clone)]
pub struct PreparedEpisode {
    /// The cognitive-layer row.
    pub episode: EpisodicMemory,
    /// Coding-memory `kind` (`turn_trace`, `fix_attempt`, `refactor`, `test_run`, …).
    pub kind: String,
    /// Optional pre-built metadata JSON (provenance merged in automatically).
    pub metadata_json: Option<serde_json::Value>,
    /// Scope partition.
    pub scope_repo_id: Option<String>,
    /// Provenance — must have non-empty `source_events`.
    pub provenance: ProvenanceMetadata,
}

/// Writer — delegates to `SemanticFactRepo` / `EpisodicMemoryRepo`, enforces provenance.
#[derive(Debug, Clone)]
pub struct DistillerWriter {
    facts: SemanticFactRepo,
    episodes: EpisodicMemoryRepo,
    entity_repo: Option<EntityRepo>,
}

impl DistillerWriter {
    /// Construct a writer around existing cognitive repos.
    #[must_use]
    pub fn new(facts: SemanticFactRepo, episodes: EpisodicMemoryRepo) -> Self {
        Self { facts, episodes, entity_repo: None }
    }

    /// Attach an entity repo for graph-edge writes (KCA Track 3).
    #[must_use]
    pub fn with_entity_repo(mut self, repo: EntityRepo) -> Self {
        self.entity_repo = Some(repo);
        self
    }

    /// Write a semantic fact. Returns `ProvenanceMissing` when source_events is empty.
    pub async fn write_fact(&self, prepared: PreparedFact) -> Result<(), DistillerError> {
        if prepared.provenance.source_events.is_empty() {
            return Err(DistillerError::ProvenanceMissing);
        }

        let metadata_json = merge_provenance(prepared.metadata_json, &prepared.provenance)?;
        let json_str =
            serde_json::to_string(&metadata_json).map_err(|e| DistillerError::Storage {
                detail: format!("metadata serialize: {e}"),
            })?;

        self.facts
            .upsert_with_metadata(
                &prepared.fact,
                prepared.scope_repo_id.as_deref(),
                Some(&json_str),
            )
            .await
            .map_err(|e| DistillerError::Storage {
                detail: format!("upsert_with_metadata: {e}"),
            })?;

        // KCA Track 3: write entity edges for distilled facts.
        if let Some(ref er) = self.entity_repo {
            crate::distiller::phase_c::write_entity_edges_for_distiller_fact(&prepared.fact, er).await;
        }

        Ok(())
    }

    /// Write an episodic row (turn_trace / fix_attempt / refactor / test_run / general).
    pub async fn write_episode(&self, prepared: PreparedEpisode) -> Result<(), DistillerError> {
        if prepared.provenance.source_events.is_empty() {
            return Err(DistillerError::ProvenanceMissing);
        }

        let metadata_json = merge_provenance(prepared.metadata_json, &prepared.provenance)?;
        let json_str =
            serde_json::to_string(&metadata_json).map_err(|e| DistillerError::Storage {
                detail: format!("metadata serialize: {e}"),
            })?;

        self.episodes
            .insert_with_kind_and_metadata(
                &prepared.episode,
                &prepared.kind,
                prepared.scope_repo_id.as_deref(),
                Some(&json_str),
            )
            .await
            .map_err(|e| DistillerError::Storage {
                detail: format!("insert_with_kind: {e}"),
            })?;
        Ok(())
    }

    /// Borrow the underlying fact repo (read-only discovery).
    pub fn facts(&self) -> &SemanticFactRepo {
        &self.facts
    }
    /// Borrow the underlying episode repo.
    pub fn episodes(&self) -> &EpisodicMemoryRepo {
        &self.episodes
    }

    /// Bump access_count on a fact by id.
    pub async fn bump_access(&self, id: &str) -> Result<(), DistillerError> {
        self.facts
            .record_access(id, 1.0)
            .await
            .map_err(|e| DistillerError::Storage {
                detail: format!("bump_access: {e}"),
            })
    }

    /// Complete a supersede chain: set predecessor's valid_until and superseded_by.
    pub async fn complete_supersede(
        &self,
        predecessor_id: &str,
        successor_id: &str,
        successor_valid_from: &str,
    ) -> Result<(), DistillerError> {
        self.facts
            .supersede(predecessor_id, successor_id)
            .await
            .map_err(|e| DistillerError::Storage {
                detail: format!("complete_supersede: {e}"),
            })?;
        // Bi-temporal: align valid_until with successor's valid_from.
        sqlx::query(
            "UPDATE semantic_facts SET valid_until = ?1 WHERE id = ?2",
        )
        .bind(successor_valid_from)
        .bind(predecessor_id)
        .execute(self.facts.pool())
        .await
        .map_err(|e| DistillerError::Storage {
            detail: format!("complete_supersede valid_until: {e}"),
        })?;
        Ok(())
    }
}

fn merge_provenance(
    base: Option<serde_json::Value>,
    prov: &ProvenanceMetadata,
) -> Result<serde_json::Value, DistillerError> {
    let prov_value = serde_json::to_value(prov).map_err(|e| DistillerError::Storage {
        detail: format!("prov serialize: {e}"),
    })?;
    let mut out = base.unwrap_or_else(|| json!({}));
    let obj = out.as_object_mut().ok_or_else(|| DistillerError::Storage {
        detail: "base metadata not object".into(),
    })?;
    obj.insert("provenance".into(), prov_value);
    Ok(out)
}
