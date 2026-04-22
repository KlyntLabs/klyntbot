//! feature-language-learning: Pronunciation coaching, practice sessions,
//! and exam tracking for English and Chinese language learning.

pub mod practice_tool;
pub mod pronunciation_provider;
pub mod types;

pub use pronunciation_provider::AppPronunciationProvider;

use std::sync::Arc;

use async_trait::async_trait;
use common::Result;
use tools_core::{FeatureMigration, FeaturePackage, HealthStatus};

#[derive(Default)]
pub struct LanguageLearningFeature;

impl LanguageLearningFeature {
    pub fn new() -> Self {
        Self
    }

    fn migration_sql() -> &'static str {
        include_str!("../migrations/001_create_tables.sql")
    }
}

pub fn language_learning_migrations() -> Vec<FeatureMigration> {
    vec![FeatureMigration {
        feature_name: "language_learning".to_string(),
        version: 1,
        description: "Create phoneme_mastery, pronunciation_logs, exam_attempts tables".to_string(),
        sql: LanguageLearningFeature::migration_sql().to_string(),
    }]
}

#[async_trait]
impl FeaturePackage for LanguageLearningFeature {
    fn name(&self) -> &str {
        "language-learning"
    }

    fn tools(&self) -> Vec<tools_core::DynTool> {
        vec![Arc::new(practice_tool::LanguagePracticeTool::new())]
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        language_learning_migrations()
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}
