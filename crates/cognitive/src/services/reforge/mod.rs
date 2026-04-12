pub mod collector;
pub mod feedback;
pub mod schema_evolution;
pub mod service;
pub mod skill_files;
pub mod types;

pub use schema_evolution::SchemaEvolutionHandler;

use std::collections::HashMap;

use async_trait::async_trait;

/// Trait for LLM-backed Reforge operations. Implemented in the agent crate.
#[async_trait]
pub trait ReforgeHandler: Send + Sync {
    /// Phase 2: Knowledge synthesis from sessions + episodics.
    async fn synthesize(
        &self,
        input: &types::SynthesizeInput,
    ) -> common::Result<types::SynthesizeOutput>;

    /// Phase 3: Skill & behavior review from corrections + routing.
    async fn review(&self, input: &types::ReviewInput) -> common::Result<types::ReviewOutput>;

    /// Phase 4: Generate human-readable narrative.
    async fn narrate(&self, input: &types::NarrateInput) -> common::Result<String>;
}

// ---------------------------------------------------------------------------
// AutotunerBridge — Phase 6 dependency inversion
// ---------------------------------------------------------------------------

/// Bridge trait for autotuner operations in Phase 6. Implemented in the agent
/// crate to avoid circular deps (cognitive L4 → autotuner L4 → agent L5).
#[async_trait]
pub trait AutotunerBridge: Send + Sync {
    /// Run trial evaluation and promotion. Returns a summary for logging.
    async fn run_evaluation(&self) -> common::Result<Phase6Result>;

    /// Create new pending trials from validated param overrides.
    async fn create_trials(&self, suggestions: Vec<ValidatedTrial>) -> common::Result<u32>;

    /// Get the current champion params as a flat key-value map.
    fn champion_params_map(&self) -> HashMap<String, f64>;

    /// Get the count of currently active trials.
    async fn active_trial_count(&self) -> u32;

    /// Expire stale trials (>7 days old, <20 messages).
    async fn expire_stale_trials(&self) -> u32;
}

/// Result of Phase 6 evaluation.
#[derive(Debug, Clone, Default)]
pub struct Phase6Result {
    pub promoted: bool,
    pub promotion_summary: Option<String>,
    pub regression: bool,
    pub evaluated_count: usize,
    pub failed_constraints: Vec<String>,
}

/// A validated trial suggestion ready for creation.
#[derive(Debug, Clone)]
pub struct ValidatedTrial {
    pub hypothesis: String,
    pub pace: String,
    pub params: HashMap<String, f64>,
}

// ---------------------------------------------------------------------------
// GraphEnrichmentHandler — Phase 6.5 dependency inversion
// ---------------------------------------------------------------------------

/// Bridge trait for LLM-based graph enrichment in Phase 6.5.
/// Implemented in the agent crate.
#[async_trait]
pub trait GraphEnrichmentHandler: Send + Sync {
    /// Run batch entity resolution and relationship discovery.
    /// Single LLM call processes all duplicate candidates and turn previews.
    async fn enrich_graph(
        &self,
        input: &crate::services::graph_enrichment::GraphEnrichmentInput,
    ) -> common::Result<crate::services::graph_enrichment::GraphEnrichmentOutput>;
}

// ---------------------------------------------------------------------------
// CommunityIntelligenceHandler — Phase 6.5 community naming/merge/split
// ---------------------------------------------------------------------------

/// Bridge trait for LLM-based community intelligence in Phase 6.5.
/// Implemented in the agent crate.
#[async_trait]
pub trait CommunityIntelligenceHandler: Send + Sync {
    /// Name communities, suggest merges and splits in a single LLM call.
    async fn analyze_communities(
        &self,
        input: &crate::services::community_intelligence::CommunityIntelligenceInput,
    ) -> common::Result<crate::services::community_intelligence::CommunityIntelligenceOutput>;
}
