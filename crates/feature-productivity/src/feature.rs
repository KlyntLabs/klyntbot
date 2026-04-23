//! ProductivityFeature — the pipeline-aware FeaturePackage for productivity.

use std::sync::Arc;

use ai_core_macros::AiFeature;
use async_trait::async_trait;
use common::Result;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

use crate::tool::ProductivityTool;

#[derive(AiFeature)]
#[ai(
    recall_domain = "Productivity",
    skill = "automation",
    tool_name = "productivity",
    entity_kind = "focus_session",
    event = "crate::events::ProductivityEvent",
)]
pub struct ProductivityFeature {
    tool: Option<DynTool>,
}

impl ProductivityFeature {
    pub fn new(tool: ProductivityTool) -> Self {
        Self {
            tool: Some(Arc::new(tool)),
        }
    }

    /// Create without a tool (for migration-only usage or tests).
    pub fn migrations_only() -> Self {
        Self { tool: None }
    }

    pub fn migration_sql() -> &'static str {
        include_str!("../migrations/001_productivity_tables.sql")
    }

    pub fn migrations_static() -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature_name: "productivity".to_string(),
            version: 2,
            description: "Create productivity tracking tables (removed legacy focus_sessions)"
                .to_string(),
            sql: Self::migration_sql().to_string(),
        }]
    }

    pub fn default_config_static() -> serde_json::Value {
        serde_json::to_value(crate::config::ProductivityConfig::default()).unwrap_or(serde_json::Value::Null)
    }
}

impl Default for ProductivityFeature {
    fn default() -> Self {
        Self::migrations_only()
    }
}

#[async_trait]
impl FeaturePackage for ProductivityFeature {
    fn name(&self) -> &str {
        "productivity"
    }

    fn tools(&self) -> Vec<DynTool> {
        self.tool.iter().cloned().collect()
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        Self::migrations_static()
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}
