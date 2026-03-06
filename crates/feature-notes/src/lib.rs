//! feature-notes: Notes and knowledge management feature package for klyntbot.

pub mod models;
pub mod repo;

use async_trait::async_trait;
use common::Result;
use serde_json::Value;
use tools_core::{FeatureMigration, FeaturePackage, HealthStatus};

pub struct NotesFeature {
    repo: repo::NoteRepo,
}

impl NotesFeature {
    pub fn new(repo: repo::NoteRepo) -> Self {
        Self { repo }
    }

    pub fn migration_sql() -> &'static str {
        include_str!("../migrations/001_create_notes.sql")
    }

    pub fn migrations_static() -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature_name: "notes".to_string(),
            version: 1,
            description: "Create notes core tables (notebooks, notes, tags, links, versions)"
                .to_string(),
            sql: Self::migration_sql().to_string(),
        }]
    }
}

#[async_trait]
impl FeaturePackage for NotesFeature {
    fn name(&self) -> &str {
        "notes"
    }

    fn tools(&self) -> Vec<tools_core::DynTool> {
        vec![] // Added in later tasks
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        Self::migrations_static()
    }

    fn config_key(&self) -> &str {
        "notes"
    }

    fn default_config(&self) -> Value {
        serde_json::json!({
            "maxVersionsPerNote": 50,
            "versionCooldownMinutes": 5
        })
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        match self.repo.check_health().await {
            Ok(()) => Ok(HealthStatus::Healthy),
            Err(e) => Ok(HealthStatus::Unhealthy(format!("DB check failed: {e}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_sql_not_empty() {
        let sql = NotesFeature::migration_sql();
        assert!(!sql.is_empty());
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS notebooks"));
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS notes"));
    }
}
