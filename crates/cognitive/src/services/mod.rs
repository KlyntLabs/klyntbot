//! Application-layer services for the cognitive memory system.
//!
//! Each module implements a distinct capability: background processing,
//! memory retrieval, consolidation, extraction, reflection, etc.
//! Domain types live in [`crate::types`], storage in [`crate::repos`].

pub mod atom_decay;
pub mod atom_extraction;
pub mod background;
pub mod compaction;
pub mod consolidation;
pub mod context_source;
pub mod conversation_recall;
pub mod decay;
pub mod extraction;
pub mod fsrs5;
pub mod louvain;
pub mod memory_promotion;
pub mod memory_retriever;
pub mod reflection;
pub mod retrieval;
pub mod salience;
pub mod scoring;
pub mod session_memory;
pub mod situation;
pub mod temporal;
pub mod tiptap_parser;
pub mod reforge;
