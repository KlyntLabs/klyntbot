//! Cognitive memory system — unified memory with FSRS decay, bi-temporal facts,
//! Mem0-style consolidation, and weekly reflection.

pub mod embedder;
pub mod repos;
pub mod search;
pub mod services;
pub mod types;

// ── Module re-exports (backward-compatible paths) ──────────
pub use services::{
    background, compaction, consolidation, context_source, conversation_recall, decay, extraction,
    memory_retriever, reflection, retrieval, salience, situation, temporal,
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
pub use extraction::{BatchExtraction, BatchExtractionResult, ExtractedFact, ExtractionHandler};
pub use memory_retriever::UnifiedMemoryService;
pub use reflection::ReflectionHandler;
pub use repos::event_log::{DomainEventRow, PipelineEventRecord, PipelineEventRow};
pub use repos::semantic_fact::DomainHealthRow;
pub use repos::{
    cognitive_migrations, AccumulatedObservationRepo, AnnotationRepo, EpisodicMemoryRepo,
    EventLogRepo, FailedObservationRepo, ProceduralRuleRepo, SemanticFactRepo,
};
pub use repos::{BlackboardEntry, BlackboardRepo, NewBlackboardEntry};
pub use repos::{
    CardType, DeckSummary, FlashcardRepo, FlashcardRow, NewFlashcard, ReviewLogEntry, ReviewQuality,
};
#[allow(deprecated)]
pub use repos::{InsightCacheRepo, InsightCacheRow};
pub use repos::AtomExtractionCache;
pub use repos::{KnowledgeAtomRepo, KnowledgeAtomRow, KnowledgeTopicRow, NewKnowledgeAtom};
pub use repos::{NewPersona, PersonaRepo, PersonaRow, PersonaUpdate};
pub use repos::{DailyReviewStat, DomainRetentionStat, ReviewStatsRepo};
pub use repos::{NewSquad, ResolvedSquad, SquadMemberRow, SquadRepo, SquadRow};
pub use situation::{compute_situation, SituationInputs, UserSituation};
pub use temporal::{ChangeSummary, FactVersion, TemporalService};
pub use types::*;
