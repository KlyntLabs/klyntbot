//! feature-todo: Self-contained task/action management feature package for klyntbot.
//!
//! Provides:
//! - `TodoFeature`: implements `FeaturePackage` (tools, migrations, config, health)
//! - `TaskTool` (alias: `TodoTool`): the primary tool with 25 actions
//! - Domain types: `Action` (alias: `Todo`), `ActionStatus` (alias: `TodoStatus`), `Attachment`, `TimeEntry`
//! - Storage: `ActionRepo`, `ActionFilter`, `ActionPatch`, `ActionSummary` + row types
//! - Trait abstractions: `EmbeddingHandler`, `EnrichmentHandler`, `ProgressHandler`
//! - Utilities: `rrule_utils`, `search`
//! - Config: `TodoConfig`

pub mod config;
pub mod embedding;
pub mod enrichment;
pub mod handler;
pub mod progress;
pub mod rrule_utils;
pub mod search;
pub mod task_complexity;
pub mod tool;
pub mod types;

pub use config::{EnrichmentConfig, SearchConfig, TodoConfig};
pub use embedding::EmbeddingHandler;
pub use enrichment::{
    EnrichmentFeedbackEntry, EnrichmentFeedbackHandler, EnrichmentHandler, EnrichmentResult,
    EnrichmentSuggestion,
};
pub use progress::ProgressHandler;
pub use rrule_utils::{humanize_rrule, next_occurrence, should_spawn_instance, validate_rrule};
pub use storage::{ActionFilter, ActionPatch, ActionRepo, ActionRow, ActionSummary};
pub use tool::{TaskTool, TodoTool};
pub use types::{
    Action, ActionStatus, Attachment, AttachmentType, TimeEntry, TimeEntrySource, Todo, TodoStatus,
};

use async_trait::async_trait;
use common::Result;
use serde_json::Value;
use std::sync::Arc;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

/// Feature package for task/action management.
pub struct TodoFeature {
    tool: Arc<TaskTool>,
}

impl TodoFeature {
    /// Create a new TodoFeature with a fully configured TaskTool.
    pub fn new(tool: TaskTool) -> Self {
        Self {
            tool: Arc::new(tool),
        }
    }

    /// Migration SQL for this feature (version 1: core tables).
    pub fn migration_sql() -> &'static str {
        include_str!("../migrations/001_create_todos.sql")
    }
}

#[async_trait]
impl FeaturePackage for TodoFeature {
    fn name(&self) -> &str {
        "todo"
    }

    fn tools(&self) -> Vec<DynTool> {
        vec![self.tool.clone()]
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature_name: "todo".to_string(),
            version: 1,
            description:
                "Create action core tables (actions, attachments, time_entries, dependencies)"
                    .to_string(),
            sql: Self::migration_sql().to_string(),
        }]
    }

    fn config_key(&self) -> &str {
        "todo"
    }

    fn default_config(&self) -> Value {
        serde_json::to_value(TodoConfig::default()).unwrap_or(Value::Null)
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        match self.tool.repo.summary().await {
            Ok(_) => Ok(HealthStatus::Healthy),
            Err(e) => Ok(HealthStatus::Unhealthy(format!("DB check failed: {}", e))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_sql_not_empty() {
        let sql = TodoFeature::migration_sql();
        assert!(!sql.is_empty());
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS actions"));
    }
}
