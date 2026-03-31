//! Configuration types for the feature-tasks crate.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::{ScopeOverrides, WorkingHours};

/// Top-level tasks feature configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct TasksConfig {
    /// Maximum number of simultaneously focused tasks.
    pub max_focus_slots: usize,
    /// Hours before a focus deadline expires.
    pub focus_deadline_hours: u64,
    /// User's local timezone string (e.g. "Asia/Bangkok").
    pub timezone: String,
    /// Enrichment engine settings.
    pub enrichment: EnrichmentConfig,
    /// Semantic search settings.
    pub search: SearchConfig,
    /// Whether to auto-log activity on task changes.
    pub auto_log_activity: bool,
    /// Whether to track estimation accuracy.
    pub estimation_tracking: bool,
    /// Default energy level for new tasks.
    pub default_energy_level: String,
    /// Whether to generate proactive suggestions.
    pub proactive_suggestions: bool,
    /// Confidence threshold for auto-applying suggestions (0.0-1.0).
    pub suggestion_auto_apply_threshold: f64,
    /// Confidence threshold for auto-applying decompositions (0.0-1.0).
    pub decomposition_auto_apply_threshold: f64,
    /// Default working hours.
    pub working_hours: WorkingHours,
    /// Maximum tasks in a day plan.
    pub max_plan_tasks: u32,
    /// Whether to auto-apply day plans.
    pub auto_apply_day_plan: bool,
    /// Work-in-progress limit.
    pub wip_limit: u32,
    /// Days after which a task is considered stale.
    pub stale_task_days: u32,
    /// Per-project overrides.
    pub project_overrides: HashMap<String, ScopeOverrides>,
    /// Per-area overrides.
    pub area_overrides: HashMap<String, ScopeOverrides>,
    /// Minimum sample size for forecasting.
    pub forecast_min_sample_size: u32,
    /// How far back (in days) to look for forecast data.
    pub forecast_lookback_days: u32,
    /// Whether to integrate with the cognitive memory system.
    pub cognitive_integration: bool,
}

impl Default for TasksConfig {
    fn default() -> Self {
        Self {
            max_focus_slots: 3,
            focus_deadline_hours: 8,
            timezone: "UTC".to_string(),
            enrichment: EnrichmentConfig::default(),
            search: SearchConfig::default(),
            auto_log_activity: true,
            estimation_tracking: true,
            default_energy_level: "medium".to_string(),
            proactive_suggestions: true,
            suggestion_auto_apply_threshold: 0.85,
            decomposition_auto_apply_threshold: 0.90,
            working_hours: WorkingHours::default(),
            max_plan_tasks: 12,
            auto_apply_day_plan: false,
            wip_limit: 5,
            stale_task_days: 14,
            project_overrides: HashMap::new(),
            area_overrides: HashMap::new(),
            forecast_min_sample_size: 5,
            forecast_lookback_days: 90,
            cognitive_integration: true,
        }
    }
}

/// Enrichment engine configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrichmentConfig {
    /// Enable auto-enrichment on task creation.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Confidence threshold for auto-applying suggestions (0.0-1.0).
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
    "all-MiniLM-L6-v2-Q".to_string()
}

fn default_rrf_k() -> u32 {
    60
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = TasksConfig::default();
        assert_eq!(cfg.max_focus_slots, 3);
        assert_eq!(cfg.focus_deadline_hours, 8);
        assert_eq!(cfg.timezone, "UTC");
        assert!(cfg.enrichment.enabled);
        assert!((cfg.enrichment.auto_apply_threshold - 0.70).abs() < f64::EPSILON);
        assert!(cfg.search.enabled);
        assert!((cfg.search.semantic_threshold - 0.5).abs() < f64::EPSILON);
        assert_eq!(cfg.search.rrf_k, 60);
        assert!(cfg.auto_log_activity);
        assert!(cfg.estimation_tracking);
        assert_eq!(cfg.default_energy_level, "medium");
        assert!(cfg.proactive_suggestions);
        assert!((cfg.suggestion_auto_apply_threshold - 0.85).abs() < f64::EPSILON);
        assert!((cfg.decomposition_auto_apply_threshold - 0.90).abs() < f64::EPSILON);
        assert_eq!(cfg.max_plan_tasks, 12);
        assert!(!cfg.auto_apply_day_plan);
        assert_eq!(cfg.wip_limit, 5);
        assert_eq!(cfg.stale_task_days, 14);
        assert!(cfg.project_overrides.is_empty());
        assert!(cfg.area_overrides.is_empty());
        assert_eq!(cfg.forecast_min_sample_size, 5);
        assert_eq!(cfg.forecast_lookback_days, 90);
        assert!(cfg.cognitive_integration);
    }

    #[test]
    fn test_serde_round_trip() {
        let cfg = TasksConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: TasksConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.max_focus_slots, cfg.max_focus_slots);
        assert_eq!(parsed.timezone, cfg.timezone);
        assert_eq!(parsed.wip_limit, cfg.wip_limit);
        assert_eq!(parsed.max_plan_tasks, cfg.max_plan_tasks);
    }

    #[test]
    fn test_deserialize_from_empty_object() {
        let cfg: TasksConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(cfg.max_focus_slots, 3);
        assert!(cfg.auto_log_activity);
    }

    #[test]
    fn test_partial_override() {
        let json = r#"{"maxFocusSlots": 5, "wipLimit": 10}"#;
        let cfg: TasksConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.max_focus_slots, 5);
        assert_eq!(cfg.wip_limit, 10);
        // Others should be defaults
        assert_eq!(cfg.focus_deadline_hours, 8);
    }
}
