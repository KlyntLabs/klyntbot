pub mod episodic_memory;
pub mod procedural_rule;
pub mod semantic_fact;

pub use episodic_memory::EpisodicMemoryRepo;
pub use procedural_rule::ProceduralRuleRepo;
pub use semantic_fact::SemanticFactRepo;

use tools_core::FeatureMigration;

/// Return cognitive feature migrations for use with `StoragePool::run_feature_migrations`.
pub fn cognitive_migrations() -> Vec<FeatureMigration> {
    vec![FeatureMigration {
        feature_name: "cognitive".to_string(),
        version: 1,
        description: "Create cognitive memory tables".to_string(),
        sql: include_str!("../../migrations/001_cognitive_tables.sql").to_string(),
    }]
}
