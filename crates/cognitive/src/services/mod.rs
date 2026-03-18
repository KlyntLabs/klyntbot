//! Application-layer services for the cognitive memory system.
//!
//! Each module implements a distinct capability: background processing,
//! memory retrieval, consolidation, extraction, reflection, etc.
//! Domain types live in [`crate::types`], storage in [`crate::repos`].

pub mod background;
pub mod compaction;
pub mod consolidation;
pub mod context_source;
pub mod conversation_recall;
pub mod decay;
pub mod extraction;
pub mod memory_retriever;
pub mod reflection;
pub mod retrieval;
pub mod salience;
pub mod situation;
pub mod temporal;
