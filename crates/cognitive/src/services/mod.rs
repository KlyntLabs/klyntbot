//! Application-layer services for the cognitive memory system.
//!
//! Each module implements a distinct capability: background processing,
//! memory retrieval, consolidation, extraction, etc.
//! Domain types live in [`crate::types`], storage in [`crate::repos`].

pub mod atom_decay;
pub mod atom_extraction;
pub mod background;
pub mod community_intelligence;
pub mod community_membership_online;
pub mod compaction;
pub mod consolidation;
pub mod context_source;
pub mod conversation_recall;
pub mod decay;
pub mod extraction;
pub mod extraction_critic;
pub mod extraction_critic_types;
pub mod fsrs5;
pub mod fsrs_optimizer;
pub mod graph_enrichment;
pub mod graph_linker;
pub mod graph_linker_types;
pub mod graph_retrieval;
pub mod hierarchical_compressor;
pub mod louvain;
pub mod memory_retriever;
pub mod micro_reforge;
pub mod micro_reforge_types;
pub mod ppr_retrieval;
pub mod predictive_cache;
pub use extraction_critic::{ExtractionCriticHandler, NoopExtractionCriticHandler};
pub use hierarchical_compressor::{
    roll_up_daily, roll_up_hourly, roll_up_weekly, HierarchicalSummarizer, Tier,
};
pub use micro_reforge::{MicroReforgeHandler, NoopMicroReforgeHandler};
pub use ppr_retrieval::{personalized_pagerank, PprConfig, CachedPprGraph, build_graph_from_entities};
pub use hierarchical_compressor::{HierarchicalSummarizer, NoopHierarchicalSummarizer, Tier, roll_up_hourly, roll_up_daily, roll_up_weekly};
pub use predictive_cache::{PredictiveCache, CacheStats, query_hash};
pub use temporal_pruner::{TemporalPrunerHandler, NoopTemporalPruner, PruneInput, PruneFactRef, PruneOutput, DropDecision, apply_prune};
pub mod reforge;
pub mod retrieval;

pub mod scoring;
pub mod session_memory;
pub mod situation;
pub mod temporal;
pub mod temporal_pruner;
pub mod tiptap_parser;
pub mod value_density;
