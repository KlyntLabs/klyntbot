use serde::{Deserialize, Serialize};

/// Configuration for background cognitive tasks (extraction, consolidation,
/// reflection, coaching reasoning).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CognitiveConfig {
    /// Model for cognitive LLM calls. Falls back to agents.defaults.model if unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Provider name override. Falls back to agents.defaults.provider if unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,

    /// Temperature for cognitive calls (default: 0.2 — low creativity, high consistency).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Max tokens per cognitive call (default: 1024).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Max tokens for reflection calls (default: 2048).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflection_max_tokens: Option<u32>,

    /// Cron expression for weekly reflection (default: "0 9 * * 1" — Monday 9am).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflection_schedule: Option<String>,

    /// Enable dynamic fact retrieval using vector search (default: true).
    #[serde(default = "default_dynamic_facts_enabled")]
    pub dynamic_facts_enabled: bool,

    /// Max static facts (identity baseline) per prompt (default: 10).
    #[serde(default = "default_static_fact_limit")]
    pub static_fact_limit: usize,

    /// Max dynamic facts (query-relevant) per prompt (default: 15).
    #[serde(default = "default_dynamic_fact_limit")]
    pub dynamic_fact_limit: usize,

    /// Number of candidate facts to fetch from vector search before FSRS re-ranking (default: 30).
    #[serde(default = "default_vector_top_k")]
    pub vector_top_k: usize,

    /// Minimum cosine similarity threshold for vector search results (default: 0.55).
    #[serde(default = "default_min_similarity")]
    pub min_similarity: f64,

    /// Minimum accumulated event occurrences before promoting to extraction (default: 5).
    #[serde(default = "default_accumulate_promote_threshold")]
    pub accumulate_promote_threshold: usize,

    /// Minimum distinct days of accumulated events before promoting (default: 3).
    #[serde(default = "default_accumulate_min_days")]
    pub accumulate_min_days: usize,

    /// Maximum FSRS stability value to prevent ranking domination (default: 30.0).
    #[serde(default = "default_max_stability")]
    pub max_stability: f64,

    /// Relevance weight for semantic similarity (default: 0.30). All 5 weights should sum to 1.0.
    #[serde(default = "default_w_semantic")]
    pub relevance_weight_semantic: f64,

    /// Relevance weight for FSRS retrievability (default: 0.20).
    #[serde(default = "default_w_retrievability")]
    pub relevance_weight_retrievability: f64,

    /// Relevance weight for fact importance (default: 0.15).
    #[serde(default = "default_w_importance")]
    pub relevance_weight_importance: f64,

    /// Relevance weight for access frequency (default: 0.10).
    #[serde(default = "default_w_frequency")]
    pub relevance_weight_frequency: f64,

    /// Relevance weight for situational boost (default: 0.25).
    #[serde(default = "default_w_situation")]
    pub relevance_weight_situation: f64,

    /// Relevance weight for temporal recency (default: 0.05).
    #[serde(default = "default_w_temporal")]
    pub relevance_weight_temporal: f64,

    /// Whether InsightForge multi-dimensional retrieval is enabled (default: true).
    #[serde(default = "default_insight_forge_enabled")]
    pub insight_forge_enabled: bool,

    /// Max sub-queries for InsightForge decomposer (default: 5).
    #[serde(default = "default_insight_forge_max_sub_queries")]
    pub insight_forge_max_sub_queries: usize,

    /// Max results per source per sub-query (default: 5).
    #[serde(default = "default_insight_forge_per_source_limit")]
    pub insight_forge_per_source_limit: usize,

    /// Hard cap on total InsightForge results (default: 15).
    #[serde(default = "default_insight_forge_total_limit")]
    pub insight_forge_total_limit: usize,

    /// Timeout ms for each domain searcher (default: 800).
    #[serde(default = "default_insight_forge_per_source_timeout_ms")]
    pub insight_forge_per_source_timeout_ms: u64,
}

impl Default for CognitiveConfig {
    fn default() -> Self {
        Self {
            model: None,
            provider: None,
            temperature: None,
            max_tokens: None,
            reflection_max_tokens: None,
            reflection_schedule: None,
            dynamic_facts_enabled: default_dynamic_facts_enabled(),
            static_fact_limit: default_static_fact_limit(),
            dynamic_fact_limit: default_dynamic_fact_limit(),
            vector_top_k: default_vector_top_k(),
            min_similarity: default_min_similarity(),
            accumulate_promote_threshold: default_accumulate_promote_threshold(),
            accumulate_min_days: default_accumulate_min_days(),
            max_stability: default_max_stability(),
            relevance_weight_semantic: default_w_semantic(),
            relevance_weight_retrievability: default_w_retrievability(),
            relevance_weight_importance: default_w_importance(),
            relevance_weight_frequency: default_w_frequency(),
            relevance_weight_situation: default_w_situation(),
            relevance_weight_temporal: default_w_temporal(),
            insight_forge_enabled: default_insight_forge_enabled(),
            insight_forge_max_sub_queries: default_insight_forge_max_sub_queries(),
            insight_forge_per_source_limit: default_insight_forge_per_source_limit(),
            insight_forge_total_limit: default_insight_forge_total_limit(),
            insight_forge_per_source_timeout_ms: default_insight_forge_per_source_timeout_ms(),
        }
    }
}

fn default_dynamic_facts_enabled() -> bool {
    true
}
fn default_static_fact_limit() -> usize {
    10
}
fn default_dynamic_fact_limit() -> usize {
    15
}
fn default_vector_top_k() -> usize {
    30
}
fn default_min_similarity() -> f64 {
    0.55
}
fn default_accumulate_promote_threshold() -> usize {
    5
}
fn default_accumulate_min_days() -> usize {
    3
}
fn default_max_stability() -> f64 {
    30.0
}
fn default_w_semantic() -> f64 {
    0.3
}
fn default_w_retrievability() -> f64 {
    0.2
}
fn default_w_importance() -> f64 {
    0.15
}
fn default_w_frequency() -> f64 {
    0.1
}
fn default_w_situation() -> f64 {
    0.25
}
fn default_w_temporal() -> f64 {
    0.05
}
fn default_insight_forge_enabled() -> bool {
    true
}
fn default_insight_forge_max_sub_queries() -> usize {
    5
}
fn default_insight_forge_per_source_limit() -> usize {
    5
}
fn default_insight_forge_total_limit() -> usize {
    15
}
fn default_insight_forge_per_source_timeout_ms() -> u64 {
    800
}
