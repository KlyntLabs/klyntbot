pub mod accumulated_observation;
pub mod annotation;
pub mod book_tree;
pub mod entity;
pub mod episodic_memory;
pub mod event_log;
pub mod failed_observation;
pub mod flashcard;
pub mod gt_link;
pub mod insight_cache;
pub mod markdown_parser;
pub mod persona;
pub mod procedural_rule;
pub mod semantic_fact;
pub mod squad;

pub use accumulated_observation::AccumulatedObservationRepo;
pub use annotation::AnnotationRepo;
pub use book_tree::SqliteBookTreeRepo;
pub use entity::{
    EntityRepo, EntityRow, GraphNeighborhood, NewEntity, NewRelationship, RelationshipRow,
};
pub use episodic_memory::EpisodicMemoryRepo;
pub use event_log::EventLogRepo;
pub use failed_observation::FailedObservationRepo;
pub use flashcard::{
    CardType, DeckSummary, FlashcardRepo, FlashcardRow, NewFlashcard, ReviewLogEntry, ReviewQuality,
};
pub use gt_link::SqliteGTLinkRepo;
#[allow(deprecated)]
pub use insight_cache::{InsightCacheRepo, InsightCacheRow};
pub use markdown_parser::parse_markdown_to_tree;
pub use persona::{NewPersona, PersonaRepo, PersonaRow, PersonaUpdate};
pub use procedural_rule::ProceduralRuleRepo;
pub use semantic_fact::SemanticFactRepo;
pub use squad::{NewSquad, ResolvedSquad, SquadMemberRow, SquadRepo, SquadRow};

use tools_core::FeatureMigration;
use tracing::warn;

use crate::types::UserModel;

/// Shared domain list for user model fields.
pub const USER_MODEL_DOMAINS: &[&str] = &[
    "identity",
    "energy",
    "work",
    "finance",
    "learning",
    "preferences",
    "general",
    "tasks",
    "coaching",
    "meta",
];

/// Shared domain list for procedural rules.
pub const RULE_DOMAINS: &[&str] = &["productivity", "tasks", "finance", "coaching", "general"];

/// Return cognitive feature migrations for use with `StoragePool::run_feature_migrations`.
pub fn cognitive_migrations() -> Vec<FeatureMigration> {
    vec![
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 8,
            description: "Cognitive tables + squads + scope columns + persona skill fields".to_string(),
            sql: include_str!("../../migrations/001_cognitive_tables.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 7,
            description: "Add BookIndex tree nodes and GT-Link tables".to_string(),
            sql: include_str!("../../migrations/002_book_index_tables.sql").to_string(),
        },
    ]
}

/// Load the full `UserModel` from a `SemanticFactRepo`.
pub async fn load_user_model(fact_repo: &SemanticFactRepo) -> UserModel {
    let mut model = UserModel::default();
    for domain in USER_MODEL_DOMAINS {
        match fact_repo.list_active(domain).await {
            Ok(facts) => match *domain {
                "identity" => model.identity = facts,
                "energy" => model.energy = facts,
                "work" => model.work = facts,
                "finance" => model.finance = facts,
                "learning" => model.learning = facts,
                "preferences" => model.preferences = facts,
                _ => model.other.extend(facts),
            },
            Err(e) => {
                warn!("Failed to load {domain} facts: {e}");
            }
        }
    }
    model
}

/// Create an in-memory SQLite pool with all cognitive migrations applied (including v5 BookIndex).
#[cfg(test)]
pub(crate) async fn cognitive_test_pool() -> sqlx::SqlitePool {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query("PRAGMA foreign_keys=ON;")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::migrate!("../storage/migrations")
        .run(&pool)
        .await
        .unwrap();
    let migrations = cognitive_migrations();
    storage::StoragePool::run_feature_migrations(&pool, &migrations)
        .await
        .unwrap();
    pool
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn book_index_tables_created() {
        let pool = cognitive_test_pool().await;

        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='book_tree_nodes'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.0, 1);

        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='entity_tree_links'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.0, 1);
    }
}
