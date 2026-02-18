//! GraphQL types for reading and writing configuration.

use async_graphql::{Object, SimpleObject};

/// Exposed config sections for the dashboard Settings UI.
/// Only non-sensitive, hot-reloadable fields are included.
#[derive(Clone)]
pub struct GqlConfig {
    pub enrichment_enabled: bool,
    pub auto_apply_threshold: f64,
    pub search_enabled: bool,
    pub semantic_threshold: f64,
    pub rrf_k: i32,
    pub embedding_model: String,
}

#[Object]
impl GqlConfig {
    /// Whether AI task enrichment is enabled.
    async fn enrichment_enabled(&self) -> bool {
        self.enrichment_enabled
    }

    /// Confidence threshold for auto-applying enrichment suggestions (0.0–1.0).
    async fn auto_apply_threshold(&self) -> f64 {
        self.auto_apply_threshold
    }

    /// Whether semantic search is enabled.
    async fn search_enabled(&self) -> bool {
        self.search_enabled
    }

    /// Minimum cosine similarity for semantic search results (0.0–1.0).
    async fn semantic_threshold(&self) -> f64 {
        self.semantic_threshold
    }

    /// Reciprocal Rank Fusion k parameter for hybrid search.
    async fn rrf_k(&self) -> i32 {
        self.rrf_k
    }

    /// Embedding model name used for semantic search.
    async fn embedding_model(&self) -> &str {
        &self.embedding_model
    }
}

impl GqlConfig {
    /// Build from the full config schema.
    pub fn from_config(cfg: &config::Config) -> Self {
        Self {
            enrichment_enabled: cfg.todo.enrichment.enabled,
            auto_apply_threshold: cfg.todo.enrichment.auto_apply_threshold,
            search_enabled: cfg.todo.search.enabled,
            semantic_threshold: cfg.todo.search.semantic_threshold,
            rrf_k: cfg.todo.search.rrf_k as i32,
            embedding_model: cfg.todo.search.embedding_model.clone(),
        }
    }
}

/// Response type for updateConfig, returning fields that actually changed.
#[derive(SimpleObject, Clone)]
pub struct GqlConfigUpdateResult {
    pub success: bool,
    pub message: String,
}
