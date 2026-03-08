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
