pub mod accumulated_observation;
pub mod ai_metric_samples;
pub mod ai_signal_index;
pub mod annotation;
pub mod atom_extraction_cache;
pub mod book_tree;
pub mod co_activation;
pub mod community;
pub use cognitive_learning::deck_preference;
pub mod enhancement_trace;
pub mod enrichment;
pub mod entity;
pub mod episodic_memory;
pub mod event_log;
pub mod extraction_critic_log;
pub mod fact_changelog;
pub mod failed_observation;
pub use cognitive_learning::flashcard;
pub use cognitive_learning::fsrs_params;
pub mod gt_link;
pub mod knowledge_atom;
pub mod markdown_parser;
pub mod pending_memory;
pub mod procedural_rule;
pub use cognitive_learning::retention_history;
pub use cognitive_learning::review_session;
pub use cognitive_learning::review_stats;
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

// Schema substrate (migrations + test pool) lives in `cognitive-schema` so every
// cognitive concern crate can share it; re-exported here for the existing paths.
pub use cognitive_schema::cognitive_migrations;
#[cfg(test)]
pub(crate) use cognitive_schema::cognitive_test_pool;

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
