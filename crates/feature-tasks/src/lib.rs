#![recursion_limit = "256"]
//! feature-tasks: task management feature package for klyntbot.
//!
//! Provides:
//! - `TasksFeature`: implements `FeaturePackage` (tools, migrations, config, health)
//! - Domain types: `Task`, `Attachment`, `TimeEntry`, etc.
//! - Handler traits: `EmbeddingHandler`, `ProgressHandler`
//! - Utilities: `complexity`, `rrule_utils`, `search`
//! - Config: `TasksConfig`

pub mod alarm_side_effects;
pub mod alarms;
pub mod complexity;
pub mod config;
pub mod events;
pub mod focus_alarms;
pub mod handlers;
pub mod recurrence_repo;
pub mod rrule_utils;
pub mod search;
pub mod tool;
pub mod types;

pub use complexity::{evaluate_task_complexity, TaskComplexitySignals};
pub use config::{SearchConfig, TasksConfig};
pub use events::TaskEvent;
pub use handlers::{EmbeddingHandler, ProgressHandler};
pub use rrule_utils::{humanize_rrule, next_occurrence, should_spawn_instance, validate_rrule};
pub use search::hybrid_merge;
pub use tool::TaskTool;
pub use types::*;

use ai_core_macros::AiFeature;
use async_trait::async_trait;
use common::Result;
use std::sync::Arc;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

/// Feature package for task management.
#[derive(AiFeature)]
#[ai(
    recall_domain = "Tasks",
    skill = "task-management",
    event = "crate::events::TaskEvent",
    recall_boost_when = "query.message.to_lowercase().contains(\"deadline\") || query.message.to_lowercase().contains(\"task\") || query.message.to_lowercase().contains(\"overdue\")",
    recall_priority_field = "priority",
    recall_recency_field = "updated_at",
    recall_status_filter = "status != \"archived\"",
    mirror_snapshot(
        name = "task_focus",
        flush_interval_secs = 3600,
        event_kinds = ["TaskFocusChanged", "TaskCompleted"],
    ),
    promotion_threshold = 3,
)]
pub struct TasksFeature {
    pool: Option<storage::StoragePool>,
    task_tool: Option<Arc<TaskTool>>,
}

impl TasksFeature {
    /// Create a new TasksFeature.
    pub fn new() -> Self {
        Self {
            pool: None,
            task_tool: None,
        }
    }

    /// Create a TasksFeature with a storage pool for health checking.
    pub fn with_pool(pool: storage::StoragePool) -> Self {
        Self {
            pool: Some(pool),
            task_tool: None,
        }
    }

    /// Attach a TaskTool to be returned by `tools()`.
    pub fn with_task_tool(mut self, tool: Arc<TaskTool>) -> Self {
        self.task_tool = Some(tool);
        self
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
        match &self.task_tool {
            Some(tool) => vec![Arc::clone(tool) as DynTool],
            None => vec![],
        }
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature_name: "tasks".to_string(),
            version: 2,
            description: "Create task tables".to_string(),
            sql: Self::migration_sql().to_string(),
        }]
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
    fn test_feature_package_tools_empty_when_no_tool_set() {
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
    fn test_default_impl() {
        let feature: TasksFeature = Default::default();
        assert_eq!(feature.name(), "tasks");
    }
}
