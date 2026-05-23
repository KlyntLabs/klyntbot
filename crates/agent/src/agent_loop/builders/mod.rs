//! Subsystem builders for [`AgentLoopBuilder`].
//!
//! Each module encapsulates the construction of a major agent subsystem,
//! keeping the top-level `build()` method a readable orchestration layer.

pub(crate) mod backfill;
pub(crate) mod cognitive;
pub(crate) mod context_sources;
pub(crate) mod query_enhancement;
pub(crate) mod runtime;
pub(crate) mod tools;
