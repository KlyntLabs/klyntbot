pub mod aggregator;
pub mod config;
pub mod focus;
pub mod repos;
pub mod tool;
pub mod tracker;
pub mod types;

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

pub use aggregator::DailyAggregator;
pub use config::ProductivityConfig;
pub use focus::FocusManager;
pub use tool::ProductivityTool;
pub use types::*;

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
            version: 1,
            description: "Create productivity tracking tables".to_string(),
            sql: Self::migration_sql().to_string(),
        }]
    }

    pub fn default_config_static() -> Value {
        serde_json::to_value(ProductivityConfig::default()).unwrap_or(Value::Null)
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

    fn config_key(&self) -> &str {
        "productivity"
    }

    fn default_config(&self) -> Value {
        Self::default_config_static()
    }

    async fn health_check(&self) -> common::Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}
