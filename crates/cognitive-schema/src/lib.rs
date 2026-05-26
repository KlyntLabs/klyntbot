//! Shared schema substrate for the cognitive crate family.
//!
//! Holds the cognitive feature migrations (the SQL DDL for every cognitive
//! table — facts, episodics, flashcards, entities, mirror, …) and the in-memory
//! test-pool helper. Schema is genuinely shared infrastructure: SQLite is one
//! database file, so any concern's repo crate can query its tables as long as
//! these migrations ran once at startup. Concern crates own their repo *code*;
//! the schema lives here so they can all depend on a single base.

use tools_core::FeatureMigration;

/// Return cognitive feature migrations for use with `StoragePool::run_feature_migrations`.
pub fn cognitive_migrations() -> Vec<FeatureMigration> {
    vec![
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 4,
            description: "Core cognitive tables".to_string(),
            sql: include_str!("../migrations/001_cognitive_tables.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive_book_index".to_string(),
            version: 1,
            description: "BookIndex tree nodes and GT-Link tables".to_string(),
            sql: include_str!("../migrations/002_book_index_tables.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive_community".to_string(),
            version: 2,
            description: "Community graph tables for Louvain community detection".to_string(),
            sql: include_str!("../migrations/004_community_graph.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive_mirror".to_string(),
            version: 1,
            description: "Mirror tables (routing snapshots, trend narratives, snippets, meta_rules, brain_versions, trial_previews)"
                .to_string(),
            sql: include_str!("../migrations/003_mirror_tables.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive_metrics".to_string(),
            version: 1,
            description: "Unified AI metric samples table".to_string(),
            sql: include_str!("../migrations/005_ai_metric_samples.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive_retrieval_index".to_string(),
            version: 1,
            description: "ai_signal_index table — retrieval-side projection of AiSignal stream"
                .to_string(),
            sql: include_str!("../migrations/006_retrieval_index.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 5,
            description: "Extend procedural_rules with effectiveness_score, stability, \
                          scope_repo_id, last_applied, application_count, metadata."
                .to_string(),
            sql: include_str!("../migrations/007_procedural_rules_extension.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive_mirror".to_string(),
            version: 2,
            description: "Add coding_alert_kind and coding_alert_severity to mirror_snippets"
                .to_string(),
            sql: include_str!("../migrations/008_mirror_coding_alert.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 6,
            description: "Edge typing for entity_relationships (causal/correlational/temporal/structural)"
                .to_string(),
            sql: include_str!("../migrations/009_edge_types.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 13,
            description: "Entity merge proposals from per-turn graph linker (KCA Track 2)"
                .to_string(),
            sql: include_str!("../migrations/010_entity_merge_proposals.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 7,
            description: "Micro-Reforge state and run audit log (KCA Track 4)".to_string(),
            sql: include_str!("../migrations/011_micro_reforge_state.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 8,
            description: "Extraction critic log for nightly re-evaluation (KCA Track 5)".to_string(),
            sql: include_str!("../migrations/012_extraction_critic_log.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 9,
            description: "Entity-community membership for online clustering (KCA Track 11)".to_string(),
            sql: include_str!("../migrations/013_entity_community_members.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 10,
            description: "Hierarchical episodic compression columns (KCA Track 8)".to_string(),
            sql: include_str!("../migrations/014_hierarchical_episodics.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 11,
            description: "Skill proposals table for memory-grounded skill discovery (KCA Track 12)".to_string(),
            sql: include_str!("../migrations/015_skill_proposals.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 12,
            description: "Episodic actor_id for cross-CLI transfer (KCA Track 10/12)".to_string(),
            sql: include_str!("../migrations/016_episodic_actor_id.sql").to_string(),
        },
    ]
}

/// Build an in-memory SQLite pool with the base storage schema and all cognitive
/// migrations applied. Shared test helper for every cognitive concern crate.
pub async fn cognitive_test_pool() -> sqlx::SqlitePool {
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
