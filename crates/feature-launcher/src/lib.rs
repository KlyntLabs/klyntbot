pub mod clipboard;
pub mod repos;
pub mod search;
pub mod services;
pub mod template;
pub mod tool;
pub mod types;
pub mod window_mgmt;

use async_trait::async_trait;
use common::Result;
use std::sync::Arc;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

pub use clipboard::ClipboardMonitor;
pub use repos::*;
pub use search::*;
pub use services::*;
pub use tool::LauncherTool;
pub use types::WindowAction;
pub use types::*;
pub use window_mgmt::global as window_manager;
pub use window_mgmt::WindowManager;

#[derive(Clone)]
pub struct LauncherToolDeps {
    pub registry: Arc<SourceRegistry>,
    pub frequency: Arc<FrequencyRepo>,
    pub pins: Arc<PinsRepo>,
}

pub struct LauncherFeature {
    tool_deps: Option<LauncherToolDeps>,
}

impl LauncherFeature {
    pub fn new() -> Self {
        Self { tool_deps: None }
    }

    pub fn with_tool_deps(deps: LauncherToolDeps) -> Self {
        Self {
            tool_deps: Some(deps),
        }
    }
}

impl Default for LauncherFeature {
    fn default() -> Self {
        Self::new()
    }
}

/// Launcher schema migrations. Free function so callers don't need to
/// instantiate the feature just to run migrations.
pub fn launcher_migrations() -> Vec<FeatureMigration> {
    vec![
        FeatureMigration {
            feature_name: "launcher".to_string(),
            version: 1,
            description: "Launcher tables: frequencies, clipboard history, FTS5".to_string(),
            sql: include_str!("../migrations/001_launcher_tables.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "launcher".to_string(),
            version: 2,
            description: "Entity attention: decay-weighted attention seconds from activity_events"
                .to_string(),
            sql: include_str!("../migrations/002_entity_attention.sql").to_string(),
        },
    ]
}

#[async_trait]
impl FeaturePackage for LauncherFeature {
    fn name(&self) -> &str {
        "launcher"
    }

    fn tools(&self) -> Vec<DynTool> {
        if let Some(deps) = &self.tool_deps {
            vec![Arc::new(LauncherTool::new(
                deps.registry.clone(),
                deps.frequency.clone(),
                deps.pins.clone(),
            ))]
        } else {
            vec![]
        }
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        launcher_migrations()
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}
