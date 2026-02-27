//! Todo system configuration: TodoConfig, enrichment, notifications, focus, search, daily planning.

use serde::{Deserialize, Serialize};

use super::core::{default_semantic_threshold, default_true};

/// Task creation mode — controls whether the agent asks for details before creating tasks
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreationMode {
    /// Ask the user for details via ask_user before creating (default)
    #[default]
    #[serde(rename = "ask-first")]
    AskFirst,
    /// Auto-enrich from conversation context, present for confirmation
    #[serde(rename = "yolo")]
    Yolo,
    /// Interactive brainstorming, one question at a time
    #[serde(rename = "party")]
    Party,
}

fn deserialize_creation_mode<'de, D>(deserializer: D) -> std::result::Result<CreationMode, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    match s.as_str() {
        "ask-first" => Ok(CreationMode::AskFirst),
        "yolo" => Ok(CreationMode::Yolo),
        "party" => Ok(CreationMode::Party),
        _ => Ok(CreationMode::AskFirst), // graceful fallback
    }
}

/// Todo system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Task creation mode: ask-first (default), yolo, or party
    #[serde(default, deserialize_with = "deserialize_creation_mode")]
    pub creation_mode: CreationMode,

    /// Suggest creating a plan when a complex task is added (default: true)
    #[serde(default = "default_true")]
    pub auto_plan_suggestion: bool,

    /// Auto-generate a plan when a complex task is focused (default: false)
    #[serde(default)]
    pub auto_plan_on_focus: bool,

    /// Complexity score threshold for plan suggestions (default: 3)
    #[serde(default = "default_plan_complexity_threshold")]
    pub plan_complexity_threshold: u8,
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

    /// Embedding model name (default: "paraphrase-multilingual-MiniLM-L12-v2")
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

impl Default for TodoConfig {
    fn default() -> Self {
        Self {
            notifications: TodoNotificationConfig::default(),
            focus: TodoFocusConfig::default(),
            enrichment: TodoEnrichmentConfig::default(),
            search: TodoSearchConfig::default(),
            daily_planning: DailyPlanningConfig::default(),
            creation_mode: CreationMode::default(),
            auto_plan_suggestion: true,
            auto_plan_on_focus: false,
            plan_complexity_threshold: default_plan_complexity_threshold(),
        }
    }
}

fn default_plan_complexity_threshold() -> u8 {
    3
}

fn default_embedding_model() -> String {
    "paraphrase-multilingual-MiniLM-L12-v2".to_string()
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
