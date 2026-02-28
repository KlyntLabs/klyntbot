//! Configuration types for the feature-todo crate.

use serde::{Deserialize, Serialize};

/// Top-level todo feature configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoConfig {
    /// Maximum number of simultaneously focused tasks.
    #[serde(default = "default_max_focus_slots")]
    pub max_focus_slots: usize,
    /// Hours before a focus deadline expires.
    #[serde(default = "default_focus_deadline_hours")]
    pub focus_deadline_hours: u64,
    /// User's local timezone string (e.g. "Asia/Bangkok").
    #[serde(default = "default_timezone")]
    pub timezone: String,
    /// Enrichment engine settings.
    #[serde(default)]
    pub enrichment: EnrichmentConfig,
    /// Semantic search settings.
    #[serde(default)]
    pub search: SearchConfig,
}

impl Default for TodoConfig {
    fn default() -> Self {
        Self {
            max_focus_slots: default_max_focus_slots(),
            focus_deadline_hours: default_focus_deadline_hours(),
            timezone: default_timezone(),
            enrichment: EnrichmentConfig::default(),
            search: SearchConfig::default(),
        }
    }
}

fn default_max_focus_slots() -> usize {
    3
}

fn default_focus_deadline_hours() -> u64 {
    8
}

fn default_timezone() -> String {
    "UTC".to_string()
}

/// Enrichment engine configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrichmentConfig {
    /// Enable auto-enrichment on task creation.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Confidence threshold for auto-applying suggestions (0.0–1.0).
    #[serde(default = "default_auto_apply_threshold")]
    pub auto_apply_threshold: f64,
}

impl Default for EnrichmentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_apply_threshold: default_auto_apply_threshold(),
        }
    }
}

fn default_auto_apply_threshold() -> f64 {
    0.70
}

fn default_true() -> bool {
    true
}

/// Semantic search configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchConfig {
    /// Enable semantic (vector) search.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Minimum cosine similarity for results.
    #[serde(default = "default_semantic_threshold")]
    pub semantic_threshold: f64,
    /// Embedding model name.
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,
    /// Reciprocal Rank Fusion k parameter.
    #[serde(default = "default_rrf_k")]
    pub rrf_k: u32,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            semantic_threshold: default_semantic_threshold(),
            embedding_model: default_embedding_model(),
            rrf_k: default_rrf_k(),
        }
    }
}

fn default_semantic_threshold() -> f64 {
    0.5
}

fn default_embedding_model() -> String {
    "paraphrase-multilingual-MiniLM-L12-v2".to_string()
}

fn default_rrf_k() -> u32 {
    60
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = TodoConfig::default();
        assert_eq!(cfg.max_focus_slots, 3);
        assert_eq!(cfg.focus_deadline_hours, 8);
        assert_eq!(cfg.timezone, "UTC");
        assert!(cfg.enrichment.enabled);
        assert!((cfg.enrichment.auto_apply_threshold - 0.70).abs() < f64::EPSILON);
        assert!(cfg.search.enabled);
        assert!((cfg.search.semantic_threshold - 0.5).abs() < f64::EPSILON);
        assert_eq!(cfg.search.rrf_k, 60);
    }

    #[test]
    fn test_serde_round_trip() {
        let cfg = TodoConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: TodoConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.max_focus_slots, cfg.max_focus_slots);
        assert_eq!(parsed.timezone, cfg.timezone);
    }

}
