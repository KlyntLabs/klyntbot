#![recursion_limit = "256"]
//! feature-tasks: Agentic task management feature package for klyntbot.
//!
//! Provides:
//! - `TasksFeature`: implements `FeaturePackage` (tools, migrations, config, health)
//! - Domain types: `Task`, `TaskActivity`, etc.
//! - Handler traits: `EmbeddingHandler`, `ProgressHandler`
//! - Utilities: `scoring`, `complexity`, `rrule_utils`, `search`
//! - Config: `TasksConfig`

pub mod alarm_side_effects;
pub mod alarms;
pub mod cognitive_bridge;
pub mod complexity;
pub mod config;
pub mod focus_alarms;
pub mod handlers;
pub mod recurrence_repo;
pub mod rrule_utils;
pub mod search;
pub mod tool;
pub mod types;

pub use complexity::{evaluate_task_complexity, TaskComplexitySignals};
pub use config::{SearchConfig, TasksConfig};
pub use handlers::{EmbeddingHandler, ProgressHandler};
pub use rrule_utils::{humanize_rrule, next_occurrence, should_spawn_instance, validate_rrule};
pub use search::hybrid_merge;
pub use tool::TaskTool;
pub use types::*;

use async_trait::async_trait;
use common::Result;
use serde_json::Value;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

/// Feature package for agentic task management.
pub struct TasksFeature {
    pool: Option<storage::StoragePool>,
}

impl TasksFeature {
    /// Create a new TasksFeature.
    pub fn new() -> Self {
        Self { pool: None }
    }

    /// Create a TasksFeature with a storage pool for health checking.
    pub fn with_pool(pool: storage::StoragePool) -> Self {
        Self { pool: Some(pool) }
    }

    /// Migration SQL for this feature (version 1: core tables).
    pub fn migration_sql() -> &'static str {
        include_str!("../migrations/001_create_tasks.sql")
    }
}

impl Default for TasksFeature {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FeaturePackage for TasksFeature {
    fn name(&self) -> &str {
        "tasks"
    }

    fn tools(&self) -> Vec<DynTool> {
        vec![] // TaskTool is wired directly in agent builder, not via FeaturePackage
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature_name: "tasks".to_string(),
            version: 2,
            description: "Create agentic task tables".to_string(),
            sql: Self::migration_sql().to_string(),
        }]
    }

    fn config_key(&self) -> &str {
        "tasks"
    }

    fn default_config(&self) -> Value {
        serde_json::to_value(TasksConfig::default()).unwrap_or(Value::Null)
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        let Some(pool) = &self.pool else {
            return Ok(HealthStatus::Healthy);
        };
        match sqlx::query("SELECT 1 FROM tasks LIMIT 1")
            .execute(pool.inner())
            .await
        {
            Ok(_) => Ok(HealthStatus::Healthy),
            Err(e) => Ok(HealthStatus::Degraded(format!(
                "tasks table unreachable: {e}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_sql_not_empty() {
        let sql = TasksFeature::migration_sql();
        assert!(!sql.is_empty());
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS tasks"));
    }

    #[test]
    fn test_feature_package_name() {
        let feature = TasksFeature::new();
        assert_eq!(feature.name(), "tasks");
    }

    #[test]
    fn test_feature_package_config_key() {
        let feature = TasksFeature::new();
        assert_eq!(feature.config_key(), "tasks");
    }

    #[test]
    fn test_feature_package_tools_empty() {
        let feature = TasksFeature::new();
        assert!(feature.tools().is_empty());
    }

    #[test]
    fn test_feature_package_migrations() {
        let feature = TasksFeature::new();
        let migrations = feature.migrations();
        assert_eq!(migrations.len(), 1);
        assert_eq!(migrations[0].feature_name, "tasks");
        assert_eq!(migrations[0].version, 2);
    }

    #[test]
    fn test_feature_package_default_config_is_valid_json() {
        let feature = TasksFeature::new();
        let config = feature.default_config();
        assert!(config.is_object());
        let obj = config.as_object().unwrap();
        assert!(obj.contains_key("maxFocusSlots"));
        assert!(obj.contains_key("wipLimit"));
        assert!(obj.contains_key("workingHours"));
    }

    #[test]
    fn test_default_impl() {
        let feature: TasksFeature = Default::default();
        assert_eq!(feature.name(), "tasks");
    }
}
