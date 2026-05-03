//! Semantic fact search handler trait — dependency inversion for
//! `MemoryTool::search_all` to query the cognitive semantic-fact pool
//! (the same pool that pre-injection retrieval uses) without taking a
//! direct dependency on the cognitive crate.
//!
//! Implemented in `agent` (L5) delegating to
//! `cognitive::services::retrieval::retrieve_relevant_facts`.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A single fact returned by semantic-fact search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactSearchResult {
    pub id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub domain: String,
    pub confidence: f64,
    pub score: f64,
}

/// Interface for searching the semantic-fact memory pool.
///
/// Defined in `tools` (L4) for use by `MemoryTool::search_all`.
/// Implemented in `agent` (L5) delegating to `retrieve_relevant_facts`.
#[async_trait]
pub trait SemanticFactSearchHandler: Send + Sync {
    /// Search the semantic-fact pool with the given query.
    ///
    /// `widen` requests broader matching: doubles `limit`, lowers
    /// similarity threshold. Used by the Tier 2 retry path on
    /// retrieval-empty refusals.
    async fn search_facts(
        &self,
        query: &str,
        limit: usize,
        widen: bool,
    ) -> common::Result<Vec<FactSearchResult>>;
}
