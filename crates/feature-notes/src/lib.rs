//! feature-notes: Notes and knowledge management feature package for klyntbot.

pub mod events;
pub mod front_matter;
pub mod handlers;
pub mod link_parser;
pub mod models;
pub mod repo;
pub mod tool;

pub use events::NoteEvent;

use ai_core_macros::AiFeature;
use async_trait::async_trait;
use common::Result;
use tools_core::{FeatureMigration, FeaturePackage, HealthStatus};

#[derive(AiFeature)]
#[ai(
    recall_domain = "Notes",
    skill = "notebook",
    tool_name = "notes",
    entity_kind = "note",
    event = "crate::events::NoteEvent"
)]
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
}

pub fn notes_migrations() -> Vec<FeatureMigration> {
    vec![
        FeatureMigration {
            feature_name: "notes".to_string(),
            version: 7,
            description:
                "Create notes core tables (notebooks, notes, tags, links, entity_mentions, versions)"
                    .to_string(),
            sql: NotesFeature::migration_sql().to_string(),
        },
        FeatureMigration {
            feature_name: "notes_practice".to_string(),
            version: 1,
            description: "Create practice_sessions table for translation practice".to_string(),
            sql: include_str!("../migrations/002_practice_sessions.sql").to_string(),
        },
    ]
}

#[async_trait]
impl FeaturePackage for NotesFeature {
    fn name(&self) -> &str {
        "notes"
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        notes_migrations()
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
