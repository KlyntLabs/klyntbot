pub mod accumulated_observation;
pub mod ai_metric_samples;
pub mod ai_signal_index;
pub mod annotation;
pub mod atom_extraction_cache;
pub mod book_tree;
pub mod co_activation;
pub mod community;
pub mod deck_preference;
pub mod enhancement_trace;
pub mod enrichment;
pub mod entity;
pub mod episodic_memory;
pub mod event_log;
pub mod extraction_critic_log;
pub mod fact_changelog;
pub mod failed_observation;
pub mod flashcard;
pub mod fsrs_params;
pub mod gt_link;
pub mod knowledge_atom;
pub mod markdown_parser;
pub mod pending_memory;
pub mod procedural_rule;
pub mod retention_history;
pub mod review_session;
pub mod review_stats;
pub mod semantic_fact;
pub use accumulated_observation::AccumulatedObservationRepo;
pub use ai_metric_samples::MetricRepo;
pub use ai_signal_index::{AiSignalIndexRepo, IndexedSignal};
pub use annotation::AnnotationRepo;
pub use atom_extraction_cache::AtomExtractionCache;
pub use book_tree::SqliteBookTreeRepo;
pub use co_activation::CoActivationRepo;
pub use community::{CommunityMemberRow, CommunityRepo, CommunityRow};
pub use deck_preference::{DeckPreferenceRepo, DeckPreferenceRow};
pub use enhancement_trace::EnhancementTraceRepo;
pub use enrichment::{
    ConversationDensityRepo, ConversationDensityRow, KnowledgeSnapshotRepo, KnowledgeSnapshotRow,
};
pub use entity::{
    EdgeRow, EntityRepo, EntityRow, GraphNeighborhood, NewEntity, NewRelationship, RelationshipRow,
};
pub use episodic_memory::EpisodicMemoryRepo;
pub use event_log::EventLogRepo;
pub use extraction_critic_log::{ExtractionCriticLogEntry, ExtractionCriticLogRepo};
pub use fact_changelog::FactChangelogRepo;
pub use failed_observation::FailedObservationRepo;
pub use flashcard::{
    CardType, DeckSummary, FlashcardRepo, FlashcardRow, NewFlashcard, ReviewLogEntry, ReviewQuality,
};
pub use fsrs_params::FsrsParamsRepo;
pub use gt_link::SqliteGTLinkRepo;
pub use knowledge_atom::{
    KnowledgeAtomRepo, KnowledgeAtomRow, KnowledgeTopicRow, NewKnowledgeAtom,
};
pub use markdown_parser::parse_markdown_to_tree;
pub use pending_memory::{PendingMemoryRepo, PendingMemoryRow};
pub use procedural_rule::ProceduralRuleRepo;
pub use retention_history::{DailyRetentionPoint, DomainRetentionHistory, RetentionHistoryRepo};
pub use review_session::{ReviewSessionRepo, ReviewSessionRow};
pub use review_stats::{DailyReviewStat, DomainRetentionStat, ReviewStatsRepo};
pub use semantic_fact::SemanticFactRepo;

use tools_core::FeatureMigration;
use tracing::warn;

use crate::types::UserModel;

/// Map a `sqlx::Error` to our domain error type.
pub(crate) fn map_sqlx(e: sqlx::Error) -> common::KlyntbotError {
    common::KlyntbotError::Storage(e.to_string())
}

/// Shared domain list for user model fields.
///
/// This is the semantic-fact taxonomy — it categorizes what a *fact about the
/// user* is about. It is **orthogonal** to `bus::EventDomain` (event category)
/// and `ai_core::RecallDomain` (AI-feature retrieval axis). Overlapping
/// strings (`finance`, `tasks`, `coaching`, ...) are coincidence, not shared
/// meaning. A fact tagged `"energy"` is not the same thing as an event
/// tagged `EventDomain::Energy`.
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

/// Shared domain list for procedural rules. Like `USER_MODEL_DOMAINS`, this is
/// a rule-side taxonomy, not the pipeline event domain.
pub const RULE_DOMAINS: &[&str] = &["productivity", "tasks", "finance", "coaching", "general"];

/// Return cognitive feature migrations for use with `StoragePool::run_feature_migrations`.
pub fn cognitive_migrations() -> Vec<FeatureMigration> {
    vec![
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 4,
            description: "Core cognitive tables".to_string(),
            sql: include_str!("../../migrations/001_cognitive_tables.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive_book_index".to_string(),
            version: 1,
            description: "BookIndex tree nodes and GT-Link tables".to_string(),
            sql: include_str!("../../migrations/002_book_index_tables.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive_community".to_string(),
            version: 2,
            description: "Community graph tables for Louvain community detection".to_string(),
            sql: include_str!("../../migrations/004_community_graph.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive_mirror".to_string(),
            version: 1,
            description: "Mirror tables (routing snapshots, trend narratives, snippets, meta_rules, brain_versions, trial_previews)"
                .to_string(),
            sql: include_str!("../../migrations/003_mirror_tables.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive_metrics".to_string(),
            version: 1,
            description: "Unified AI metric samples table".to_string(),
            sql: include_str!("../../migrations/005_ai_metric_samples.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive_retrieval_index".to_string(),
            version: 1,
            description: "ai_signal_index table — retrieval-side projection of AiSignal stream"
                .to_string(),
            sql: include_str!("../../migrations/006_retrieval_index.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 5,
            description: "Extend procedural_rules with effectiveness_score, stability, \
                          scope_repo_id, last_applied, application_count, metadata."
                .to_string(),
            sql: include_str!("../../migrations/007_procedural_rules_extension.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive_mirror".to_string(),
            version: 2,
            description: "Add coding_alert_kind and coding_alert_severity to mirror_snippets"
                .to_string(),
            sql: include_str!("../../migrations/008_mirror_coding_alert.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 6,
            description: "Edge typing for entity_relationships (causal/correlational/temporal/structural)"
                .to_string(),
            sql: include_str!("../../migrations/009_edge_types.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 13,
            description: "Entity merge proposals from per-turn graph linker (KCA Track 2)"
                .to_string(),
            sql: include_str!("../../migrations/010_entity_merge_proposals.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 7,
            description: "Micro-Reforge state and run audit log (KCA Track 4)".to_string(),
            sql: include_str!("../../migrations/011_micro_reforge_state.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 8,
            description: "Extraction critic log for nightly re-evaluation (KCA Track 5)".to_string(),
            sql: include_str!("../../migrations/012_extraction_critic_log.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 9,
            description: "Entity-community membership for online clustering (KCA Track 11)".to_string(),
            sql: include_str!("../../migrations/013_entity_community_members.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 10,
            description: "Hierarchical episodic compression columns (KCA Track 8)".to_string(),
            sql: include_str!("../../migrations/014_hierarchical_episodics.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 11,
            description: "Skill proposals table for memory-grounded skill discovery (KCA Track 12)".to_string(),
            sql: include_str!("../../migrations/015_skill_proposals.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 12,
            description: "Episodic actor_id for cross-CLI transfer (KCA Track 10/12)".to_string(),
            sql: include_str!("../../migrations/016_episodic_actor_id.sql").to_string(),
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
