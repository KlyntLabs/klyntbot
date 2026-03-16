pub mod clipboard;
pub mod repos;
pub mod search;
pub mod types;
pub mod window_mgmt;

use async_trait::async_trait;
use common::Result;
use serde_json::Value;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

pub use clipboard::ClipboardMonitor;
pub use repos::*;
pub use search::*;
pub use types::*;
pub use window_mgmt::WindowManager;

pub struct LauncherFeature;

impl LauncherFeature {
    pub fn new() -> Self {
        Self
    }

    pub fn migration_sql() -> &'static str {
        include_str!("../migrations/001_launcher_tables.sql")
    }

    pub fn migrations_static() -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature_name: "launcher".to_string(),
            version: 1,
            description: "Launcher tables: frequencies, clipboard history, FTS5".to_string(),
            sql: Self::migration_sql().to_string(),
        }]
    }
}

impl Default for LauncherFeature {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FeaturePackage for LauncherFeature {
    fn name(&self) -> &str {
        "launcher"
    }

    fn tools(&self) -> Vec<DynTool> {
        vec![]
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        Self::migrations_static()
    }

    fn config_key(&self) -> &str {
        "launcher"
    }

    fn default_config(&self) -> Value {
        serde_json::json!({
            "enabled": true,
            "clipboardHistoryEnabled": true,
            "clipboardMaxEntries": 1000,
            "scriptsDir": "~/.klyntbot/scripts"
        })
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}
