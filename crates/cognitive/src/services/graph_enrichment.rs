//! Types and traits for batch graph enrichment.
//!
//! Phase 6.5 collects medium-density conversation turns and runs LLM-based
//! entity resolution in a single batch call. The `GraphEnrichmentHandler`
//! trait is implemented in the agent crate (dependency inversion).

/// A candidate entity pair for deduplication.
#[derive(Debug, Clone)]
pub struct DuplicateCandidate {
    pub entity_a_id: String,
    pub entity_b_id: String,
    pub entity_a_name: String,
    pub entity_b_name: String,
}

/// LLM decision for a duplicate pair.
#[derive(Debug, Clone)]
pub struct MergeDecision {
    pub entity_a_id: String,
    pub entity_b_id: String,
    /// `true` = merge (a absorbs b), `false` = keep separate.
    pub should_merge: bool,
    /// The canonical name to use after merge.
    pub canonical_name: Option<String>,
    pub reason: String,
}

/// An entity relationship discovered from conversation context.
#[derive(Debug, Clone)]
pub struct DiscoveredRelationship {
    pub source_entity_name: String,
    pub target_entity_name: String,
    pub relationship_type: String,
    pub strength: f64,
}

/// Input for batch graph enrichment (Phase 6.5).
#[derive(Debug, Clone)]
pub struct GraphEnrichmentInput {
    /// Medium-density conversation turn previews to extract relationships from.
    pub turn_previews: Vec<String>,
    /// Duplicate candidate pairs for entity resolution.
    pub duplicate_candidates: Vec<DuplicateCandidate>,
}

/// Output from batch graph enrichment (Phase 6.5).
#[derive(Debug, Clone, Default)]
pub struct GraphEnrichmentOutput {
    pub merge_decisions: Vec<MergeDecision>,
    pub discovered_relationships: Vec<DiscoveredRelationship>,
}

/// Graph quality metrics computed after Phase 6.5.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct GraphQualityMetrics {
    pub entity_count: u32,
    pub relationship_count: u32,
    pub orphan_entity_count: u32,
    pub orphan_rate: f64,
    pub avg_degree: f64,
    pub merge_count: u32,
    pub new_relationships: u32,
}
