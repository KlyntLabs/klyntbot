//! Trait for embedding semantic facts into vector storage.
//!
//! Defined in `cognitive` (L5), implemented in `agent` (L5) via
//! dependency inversion — same pattern as `ExtractionHandler` and
//! `ConsolidationHandler`.

use async_trait::async_trait;

use crate::types::SemanticFact;

/// Embeds semantic facts into vector storage for similarity search.
///
/// Implementations handle:
/// - Generating 384-dim embeddings from SPO triple text
/// - Storing/removing vectors in LanceDB
/// - Searching by cosine similarity with domain pre-filtering
#[async_trait]
pub trait SemanticFactEmbedder: Send + Sync {
    /// Embed a semantic fact and store its vector in LanceDB.
    ///
    /// Text formula: `"{subject} {predicate} {object}"`.
    /// Called after every consolidation upsert (fire-and-forget).
    async fn embed_and_store_fact(&self, fact: &SemanticFact) -> common::Result<()>;

    /// Remove the embedding for a superseded/archived fact.
    async fn remove_embedding(&self, fact_id: &str) -> common::Result<()>;

    /// Search for facts similar to a query, pre-filtered by domains.
    ///
    /// Returns `(fact_id, cosine_similarity)` pairs sorted by similarity desc.
    /// Only results with similarity >= `min_similarity` are returned.
    async fn search_similar(
        &self,
        query: &str,
        domains: &[&str],
        top_k: usize,
        min_similarity: f64,
    ) -> common::Result<Vec<(String, f64)>>;

    /// Re-embed all provided facts (backfill/reindex).
    ///
    /// Returns the number of facts successfully embedded.
    async fn reindex_all(&self, facts: &[SemanticFact]) -> common::Result<usize>;

    /// Whether the embedding engine is loaded and available.
    fn is_available(&self) -> bool;
}
