//! Cognitive memory system — unified memory with FSRS decay, bi-temporal facts,
//! Mem0-style consolidation, and weekly reflection.

pub mod background;
pub mod compaction;
pub mod consolidation;
pub mod context_source;
pub mod conversation_recall;
pub mod decay;
pub mod embedder;
pub mod extraction;
pub mod memory_retriever;
pub mod reflection;
pub mod repos;
pub mod retrieval;
pub mod salience;
pub mod search;
pub mod situation;
pub mod types;

pub use background::PipelineEvent;
pub use compaction::CompactionResult;
pub use consolidation::ConsolidationHandler;
pub use context_source::{CognitiveContextSource, CognitiveRetrievalConfig};
pub use conversation_recall::{
    ConversationRecallService, RecallConfig, RecallMetadata, RecallResult,
};
pub use embedder::{SemanticFactEmbedder, TextEmbedder};
pub use extraction::{ExtractedFact, ExtractionHandler};
pub use memory_retriever::UnifiedMemoryService;
pub use reflection::ReflectionHandler;
pub use repos::event_log::{DomainEventRow, PipelineEventRow};
pub use repos::{
    cognitive_migrations, AccumulatedObservationRepo, AnnotationRepo, EpisodicMemoryRepo,
    EventLogRepo, ProceduralRuleRepo, SemanticFactRepo,
};
pub use situation::{compute_situation, SituationInputs, UserSituation};
pub use types::*;
