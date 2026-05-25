//! ProductivityFeature — the pipeline-aware FeaturePackage for productivity.

use ai_core_macros::AiFeature;
use async_trait::async_trait;
use common::Result;
use tools_core::{FeatureMigration, FeaturePackage, HealthStatus};

#[derive(AiFeature, Default)]
#[ai(
    recall_domain = "Productivity",
    skill = "automation",
    tool_name = "productivity",
    entity_kind = "focus_session",
    event = "crate::events::ProductivityEvent"
)]
pub struct ProductivityFeature;

/// Productivity schema migrations. Free function so callers don't need to
/// instantiate the feature just to run migrations.
pub fn productivity_migrations() -> Vec<FeatureMigration> {
    vec![
        FeatureMigration {
            feature_name: "productivity".to_string(),
            version: 2,
            description: "Create productivity tracking tables (removed legacy focus_sessions)"
                .to_string(),
            sql: include_str!("../migrations/001_productivity_tables.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "productivity".to_string(),
            version: 3,
            description: "Fix auto-detected session source values in productivity_sessions"
                .to_string(),
            sql: include_str!("../migrations/003_fix_auto_source_values.sql").to_string(),
        },
    ]
}

#[async_trait]
impl FeaturePackage for ProductivityFeature {
    fn name(&self) -> &str {
        "productivity"
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        productivity_migrations()
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}
