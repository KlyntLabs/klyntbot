pub mod accumulated_observation;
pub mod annotation;
pub mod episodic_memory;
pub mod event_log;
pub mod failed_observation;
pub mod procedural_rule;
pub mod semantic_fact;

pub use accumulated_observation::AccumulatedObservationRepo;
pub use annotation::AnnotationRepo;
pub use episodic_memory::EpisodicMemoryRepo;
pub use event_log::EventLogRepo;
pub use failed_observation::FailedObservationRepo;
pub use procedural_rule::ProceduralRuleRepo;
pub use semantic_fact::SemanticFactRepo;

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
    vec![FeatureMigration {
        feature_name: "cognitive".to_string(),
        version: 1,
        description: "Cognitive memory system tables".to_string(),
        sql: include_str!("../../migrations/001_cognitive_tables.sql").to_string(),
    }]
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

/// Create an in-memory SQLite pool with all cognitive migrations applied.
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
