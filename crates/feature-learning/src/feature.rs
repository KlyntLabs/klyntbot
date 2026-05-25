//! LearningFeature — flashcard + knowledge-atom feature package.
//! Owns no tables in v3 (uses cognitive's flashcards/knowledge_atoms tables).

use ai_core_macros::AiFeature;
use async_trait::async_trait;
use common::Result;
use tools_core::{FeatureMigration, FeaturePackage, HealthStatus};

#[derive(AiFeature, Default)]
#[ai(
    recall_domain = "Learning",
    skill = "learning",
    tool_name = "learning",
    entity_kind = "knowledge_atom",
    event = "crate::events::LearningEvent"
)]
pub struct LearningFeature {
    pool: Option<storage::StoragePool>,
}

impl LearningFeature {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_pool(pool: storage::StoragePool) -> Self {
        Self { pool: Some(pool) }
    }

    pub fn migration_sql() -> &'static str {
        include_str!("../migrations/001_create_learning.sql")
    }
}

#[async_trait]
impl FeaturePackage for LearningFeature {
    fn name(&self) -> &str {
        "learning"
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature_name: "learning".to_string(),
            version: 1,
            description: "Placeholder: tables owned by cognitive in v3".to_string(),
            sql: Self::migration_sql().to_string(),
        }]
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        let Some(pool) = &self.pool else {
            return Ok(HealthStatus::Healthy);
        };
        match sqlx::query("SELECT 1 FROM knowledge_atoms LIMIT 1")
            .execute(pool.inner())
            .await
        {
            Ok(_) => Ok(HealthStatus::Healthy),
            Err(e) => Ok(HealthStatus::Degraded(format!(
                "knowledge_atoms unreachable: {e}"
            ))),
        }
    }
}
