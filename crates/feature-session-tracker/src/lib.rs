pub mod context_builder;
pub mod discovery;
pub mod injector;
pub mod parser;
pub mod repos;
pub mod types;
pub mod watcher;

use async_trait::async_trait;
use common::Result;
use serde_json::Value;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

pub struct SessionTrackerFeature;

impl SessionTrackerFeature {
    fn migration_sql() -> &'static str {
        include_str!("../migrations/001_create_session_tracker.sql")
    }

    pub fn migrations_static() -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature_name: "session_tracker".to_string(),
            version: 1,
            description: "Create session tracker tables".to_string(),
            sql: Self::migration_sql().to_string(),
        }]
    }
}

#[async_trait]
impl FeaturePackage for SessionTrackerFeature {
    fn name(&self) -> &str {
        "session_tracker"
    }

    fn tools(&self) -> Vec<DynTool> {
        vec![]
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        Self::migrations_static()
    }

    fn config_key(&self) -> &str {
        "sessionTracker"
    }

    fn default_config(&self) -> Value {
        serde_json::json!({
            "enabled": true,
            "claudeDir": "~/.claude",
            "windowSize": 30,
            "chunkSize": 50
        })
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}
