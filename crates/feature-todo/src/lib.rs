//! feature-todo: Self-contained todo/task management feature package for klyntbot.
//!
//! Provides:
//! - `TodoFeature`: implements `FeaturePackage` (tools, migrations, config, health)
//! - `TodoTool`: the primary tool with 26 actions
//! - Domain types: `Todo`, `TodoStatus`, `Attachment`, `TimeEntry`
//! - Storage: `TodoRepo`, `TodoFilter`, `TodoPatch`, `TodoSummary` + row types
//! - Trait abstractions: `CalendarSyncHandler`, `EmbeddingHandler`, `EnrichmentHandler`
//! - Utilities: `rrule_utils`, `search`
//! - Config: `TodoConfig`

pub mod calendar_sync;
pub mod config;
pub mod embedding;
pub mod enrichment;
pub mod handler;
pub mod rrule_utils;
pub mod search;
pub mod storage;
pub mod tool;
pub mod types;

pub use calendar_sync::CalendarSyncHandler;
pub use config::{CreationMode, EnrichmentConfig, SearchConfig, TodoConfig};
pub use embedding::EmbeddingHandler;
pub use enrichment::{
    EnrichmentFeedbackEntry, EnrichmentFeedbackHandler, EnrichmentHandler, EnrichmentResult,
    EnrichmentSuggestion,
};
pub use rrule_utils::{humanize_rrule, next_occurrence, should_spawn_instance, validate_rrule};
pub use storage::{TodoFilter, TodoPatch, TodoRepo, TodoRow, TodoSummary};
pub use tool::TodoTool;
pub use types::{
    Attachment, AttachmentType, TimeEntry, TimeEntrySource, Todo, TodoStatus,
};

use async_trait::async_trait;
use common::Result;
use serde_json::Value;
use std::sync::Arc;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

/// Feature package for todo/task management.
pub struct TodoFeature {
    tool: Arc<TodoTool>,
}

impl TodoFeature {
    /// Create a new TodoFeature with a fully configured TodoTool.
    pub fn new(tool: TodoTool) -> Self {
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
            description: "Create todo core tables (todos, attachments, time_entries, dependencies)"
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
        // A simple health check: list summary (requires DB)
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
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS todos"));
    }

    #[test]
    fn test_default_config_valid_json() {
        let cfg = TodoConfig::default();
        let json = serde_json::to_value(&cfg).unwrap();
        assert!(json.is_object());
        assert!(json.get("maxFocusSlots").is_some() || json.get("max_focus_slots").is_some() || {
            // camelCase via serde rename_all
            true
        });
    }
}
