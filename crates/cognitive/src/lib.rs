//! Cognitive memory system — unified memory with FSRS decay, bi-temporal facts,
//! and Mem0-style consolidation.

pub mod consumers;
pub mod embedder;
pub mod mirror;
pub mod pipeline;
pub mod repos;
pub mod search;
pub mod services;
pub mod specta_helpers;
pub mod types;

// ── Module re-exports (backward-compatible paths) ──────────
pub use services::{
    background, compaction, consolidation, context_source, conversation_recall, decay, extraction,
    memory_retriever, retrieval, session_memory, situation, temporal,
};

// ── Type re-exports ────────────────────────────────────────
pub use background::PipelineEvent;
pub use compaction::CompactionResult;
pub use consolidation::{execute_memory_ops, ConsolidationCandidate, ConsolidationHandler};
pub use context_source::{CognitiveContextSource, CognitiveRetrievalConfig};
pub use conversation_recall::{
    ConversationRecallService, RecallConfig, RecallMetadata, RecallResult,
};
pub use embedder::{SemanticFactEmbedder, TextEmbedder};
pub use extraction::{
    BatchExtraction, BatchExtractionResult, ExtractedEntity, ExtractedFact, ExtractedRelationship,
    ExtractionHandler,
};
pub use memory_retriever::UnifiedMemoryService;
pub use repos::event_log::{DomainEventRow, PipelineEventRecord, PipelineEventRow};
pub use repos::persona_accuracy::alignment_to_fsrs_rating;
pub use repos::semantic_fact::DomainHealthRow;
pub use repos::AtomExtractionCache;
pub use repos::FsrsParamsRepo;
pub use repos::{
    cognitive_migrations, AccumulatedObservationRepo, AiSignalIndexRepo, AnnotationRepo,
    CoActivationRepo, CommunityRepo, ConversationDensityRepo, EntityRepo, EpisodicMemoryRepo,
    EventLogRepo, FactChangelogRepo, FailedObservationRepo, IndexedSignal, KnowledgeSnapshotRepo,
    MetricRepo, ProceduralRuleRepo, SemanticFactRepo,
};
pub use repos::{BlackboardEntry, BlackboardRepo, NewBlackboardEntry};
pub use repos::{
    CardType, DeckSummary, FlashcardRepo, FlashcardRow, NewFlashcard, ReviewLogEntry, ReviewQuality,
};
pub use repos::{DailyRetentionPoint, DomainRetentionHistory, RetentionHistoryRepo};
pub use repos::{DailyReviewStat, DomainRetentionStat, ReviewStatsRepo};
pub use repos::{DeckPreferenceRepo, DeckPreferenceRow};
pub use repos::{KnowledgeAtomRepo, KnowledgeAtomRow, KnowledgeTopicRow, NewKnowledgeAtom};
pub use repos::{NewPersona, PersonaRepo, PersonaRow, PersonaUpdate};
pub use repos::{NewSquad, ResolvedSquad, SquadMemberRow, SquadRepo, SquadRow};
pub use repos::{PersonaAccuracy, PersonaAccuracyRepo};
pub use repos::{ReviewSessionRepo, ReviewSessionRow};
pub use session_memory::{SessionMemoryConfig, SessionMemoryService};
pub use situation::{compute_situation, SituationInputs, UserSituation};
pub use temporal::{ChangeSummary, FactVersion, TemporalService};
pub use types::*;
