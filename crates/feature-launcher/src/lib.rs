pub mod clipboard;
pub mod migration;
pub mod repos;
pub mod search;
pub mod services;
pub mod template;
pub mod tool;
pub mod types;
pub mod window_mgmt;

use async_trait::async_trait;
use common::Result;
use tools_core::{FeatureMigration, FeaturePackage, HealthStatus};

pub use clipboard::ClipboardMonitor;
pub use migration::migrate_app_ids_to_bundle_ids;
pub use repos::*;
#[doc(hidden)]
pub use search::running_apps::apply_snapshot as apply_running_snapshot_for_bench;
pub use search::*;
pub use services::*;
pub use tool::LauncherTool;
pub use types::WindowAction;
pub use types::*;
pub use window_mgmt::global as window_manager;
pub use window_mgmt::WindowManager;

/// Feature handle for launcher migrations + health. Tools are registered
/// imperatively in `LauncherPlugin::init` (the launcher tool needs the live
/// search engine), so this carries no tool state.
pub struct LauncherFeature;

impl LauncherFeature {
    pub fn new() -> Self {
        Self
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

    fn migrations(&self) -> Vec<FeatureMigration> {
        launcher_migrations()
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}
