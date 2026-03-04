pub mod config;
pub mod focus;
pub mod repos;
pub mod tracker;
pub mod types;

use async_trait::async_trait;
use serde_json::Value;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

pub use config::ProductivityConfig;
pub use focus::FocusManager;
pub use types::*;

pub struct ProductivityFeature {
    // Will hold the tool once we create it in Phase 3
}

impl ProductivityFeature {
    pub fn new() -> Self {
        Self {}
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
        Self::new()
    }
}

#[async_trait]
impl FeaturePackage for ProductivityFeature {
    fn name(&self) -> &str {
        "productivity"
    }

    fn tools(&self) -> Vec<DynTool> {
        vec![] // Will be populated in Phase 3
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
