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
    /// Semantic search settings.
    pub search: SearchConfig,
    /// Whether to auto-log activity on task changes.
    pub auto_log_activity: bool,
    /// Whether to track estimation accuracy.
    pub estimation_tracking: bool,
    /// Default energy level for new tasks.
    pub default_energy_level: String,
    /// Default working hours.
    pub working_hours: WorkingHours,
    /// Work-in-progress limit.
    pub wip_limit: u32,
    /// Days after which a task is considered stale.
    pub stale_task_days: u32,
    /// Per-project overrides.
    pub project_overrides: HashMap<String, ScopeOverrides>,
    /// Per-area overrides.
    pub area_overrides: HashMap<String, ScopeOverrides>,
    /// Whether to integrate with the cognitive memory system.
    pub cognitive_integration: bool,
}

impl Default for TasksConfig {
    fn default() -> Self {
        Self {
            max_focus_slots: 3,
            focus_deadline_hours: 8,
            timezone: "UTC".to_string(),
            search: SearchConfig::default(),
            auto_log_activity: true,
            estimation_tracking: true,
            default_energy_level: "medium".to_string(),
            working_hours: WorkingHours::default(),
            wip_limit: 5,
            stale_task_days: 14,
            project_overrides: HashMap::new(),
            area_overrides: HashMap::new(),
            cognitive_integration: true,
        }
    }
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
    "bge-small-en-v1.5-Q".to_string()
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
        assert!(cfg.search.enabled);
        assert!((cfg.search.semantic_threshold - 0.5).abs() < f64::EPSILON);
        assert_eq!(cfg.search.rrf_k, 60);
        assert!(cfg.auto_log_activity);
        assert!(cfg.estimation_tracking);
        assert_eq!(cfg.default_energy_level, "medium");
        assert_eq!(cfg.wip_limit, 5);
        assert_eq!(cfg.stale_task_days, 14);
        assert!(cfg.project_overrides.is_empty());
        assert!(cfg.area_overrides.is_empty());
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
