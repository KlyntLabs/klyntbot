//! Todo system configuration: TodoConfig, enrichment, notifications, focus, search, daily planning.

use serde::{Deserialize, Serialize};

use super::core::{default_semantic_threshold, default_true};

/// Todo system configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoConfig {
    #[serde(default)]
    pub notifications: TodoNotificationConfig,
    #[serde(default)]
    pub focus: TodoFocusConfig,
    #[serde(default)]
    pub enrichment: TodoEnrichmentConfig,
    #[serde(default)]
    pub search: TodoSearchConfig,
    #[serde(default)]
    pub daily_planning: DailyPlanningConfig,
}

/// Smart enrichment configuration for auto-inferring task metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoEnrichmentConfig {
    /// Enable/disable automatic enrichment on task creation (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Confidence threshold for auto-applying suggestions without confirmation (default: 0.85)
    #[serde(default = "default_enrichment_confidence_threshold")]
    pub auto_apply_threshold: f64,
    /// Use LLM for enrichment instead of keyword matching (opt-in, default: false)
    #[serde(default)]
    pub use_llm: bool,
}

impl Default for TodoEnrichmentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_apply_threshold: default_enrichment_confidence_threshold(),
            use_llm: false,
        }
    }
}

fn default_enrichment_confidence_threshold() -> f64 {
    0.85
}

/// Todo notification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoNotificationConfig {
    #[serde(default = "default_notification_targets")]
    pub targets: Vec<String>,
    #[serde(default = "default_true")]
    pub focus_reminders: bool,
    #[serde(default = "default_true")]
    pub daily_digest: bool,
    #[serde(default = "default_digest_time")]
    pub daily_digest_time: String,
}

impl Default for TodoNotificationConfig {
    fn default() -> Self {
        Self {
            targets: vec!["os_native".to_string()],
            focus_reminders: true,
            daily_digest: true,
            daily_digest_time: default_digest_time(),
        }
    }
}

/// Todo focus mode configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoFocusConfig {
    #[serde(default = "default_max_slots")]
    pub max_slots: usize,
    #[serde(default = "default_deadline_hours")]
    pub deadline_hours: u64,
}

impl Default for TodoFocusConfig {
    fn default() -> Self {
        Self {
            max_slots: default_max_slots(),
            deadline_hours: default_deadline_hours(),
        }
    }
}

fn default_notification_targets() -> Vec<String> {
    vec!["os_native".to_string()]
}

fn default_digest_time() -> String {
    "09:00".to_string()
}

fn default_max_slots() -> usize {
    3
}

fn default_deadline_hours() -> u64 {
    18
}

/// Semantic search configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoSearchConfig {
    /// Enable semantic search (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Cosine similarity threshold for semantic search results (0.0-1.0, default: 0.5)
    #[serde(default = "default_semantic_threshold")]
    pub semantic_threshold: f64,

    /// Embedding model name (default: "bge-small-en-v1.5-Q")
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,

    /// RRF k parameter for hybrid search (default: 60)
    #[serde(default = "default_rrf_k")]
    pub rrf_k: u32,
}

impl Default for TodoSearchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            semantic_threshold: default_semantic_threshold(),
            embedding_model: default_embedding_model(),
            rrf_k: default_rrf_k(),
        }
    }
}

fn default_embedding_model() -> String {
    "bge-small-en-v1.5-Q".to_string()
}

fn default_rrf_k() -> u32 {
    60
}

fn default_planning_time() -> String {
    "08:00".to_string()
}

/// Daily planning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyPlanningConfig {
    /// Enable/disable daily planning feature (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Time to trigger daily planning in HH:MM format (default: "08:00")
    #[serde(default = "default_planning_time")]
    pub planning_time: String,
}

impl Default for DailyPlanningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            planning_time: "08:00".to_string(),
        }
    }
}
