//! Application-layer services for the cognitive memory system.
//!
//! Each module implements a distinct capability: background processing,
//! memory retrieval, consolidation, extraction, etc.
//! Domain types live in [`crate::types`], storage in [`crate::repos`].

pub mod atom_decay;
pub mod atom_extraction;
pub mod background;
pub use cognitive_graph::community_intelligence;
pub use cognitive_graph::community_membership_online;
pub mod compaction;
pub mod consolidation;
pub mod context_source;
pub mod conversation_recall;
pub mod decay;
pub mod extraction;
pub mod extraction_critic;
pub mod extraction_critic_types;
pub use cognitive_learning::fsrs5;
pub use cognitive_learning::fsrs_optimizer;
pub use cognitive_graph::graph_enrichment;
pub use cognitive_graph::graph_linker;
pub use cognitive_graph::graph_linker_types;
pub use cognitive_graph::graph_retrieval;
pub mod hierarchical_compressor;
pub use cognitive_graph::louvain;
pub mod memory_retriever;
pub mod micro_reforge;
pub mod micro_reforge_types;
pub mod ppr_retrieval;
pub mod predictive_cache;
pub use extraction_critic::{ExtractionCriticHandler, NoopExtractionCriticHandler};
pub use hierarchical_compressor::{
    roll_up_daily, roll_up_hourly, roll_up_weekly, HierarchicalSummarizer,
    NoopHierarchicalSummarizer, Tier,
};
pub use micro_reforge::{MicroReforgeHandler, NoopMicroReforgeHandler};
pub use ppr_retrieval::{
    build_graph_from_entities, personalized_pagerank, CachedPprGraph, PprConfig,
};
pub use predictive_cache::{query_hash, CacheStats, PredictiveCache};
pub use temporal_pruner::{
    apply_prune, DropDecision, NoopTemporalPruner, PruneFactRef, PruneInput, PruneOutput,
    TemporalPrunerHandler,
};
pub mod reforge;
pub mod retrieval;

pub mod scoring;
pub mod session_memory;
pub mod situation;
pub mod temporal;
pub mod temporal_pruner;
pub mod value_density;
