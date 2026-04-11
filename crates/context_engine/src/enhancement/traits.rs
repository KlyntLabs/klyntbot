use async_trait::async_trait;

use crate::memory_retriever::MemoryEntry;
use crate::rewriter::RetrievalContext;

use super::types::{EnhancementBudget, QueryBundle, QuerySource};

// ---------------------------------------------------------------------------
// QueryStage
// ---------------------------------------------------------------------------

/// A single query-transformation step in the enhancement pipeline.
///
/// Stages receive a [`QueryBundle`] and return a (possibly modified) bundle.
/// Each stage is responsible for staying within the constraints of the
/// supplied [`EnhancementBudget`] and for skipping itself gracefully when
/// the budget does not allow it to run.
#[async_trait]
pub trait QueryStage: Send + Sync {
    /// The identifier for this stage, used in tracing and budget accounting.
    fn name(&self) -> QuerySource;

    /// Transform the incoming query bundle.
    ///
    /// Implementations should not panic on budget exhaustion; instead they
    /// should return the input bundle unchanged or a minimally modified
    /// version, and record the skip in the trace externally via
    /// [`EnhancementTrace`](super::types::EnhancementTrace).
    async fn transform(
        &self,
        input: QueryBundle,
        context: &RetrievalContext,
        budget: &EnhancementBudget,
    ) -> common::Result<QueryBundle>;
}

// ---------------------------------------------------------------------------
// RankingStage
// ---------------------------------------------------------------------------

/// A candidate re-ranking step in the enhancement pipeline.
///
/// Ranking stages receive the query bundle and a set of retrieved
/// [`MemoryEntry`] candidates, and return them in a new order.
#[async_trait]
pub trait RankingStage: Send + Sync {
    /// The identifier for this stage, used in tracing and budget accounting.
    fn name(&self) -> QuerySource;

    /// Re-rank the supplied candidates relative to the query.
    async fn rerank(
        &self,
        query: &QueryBundle,
        candidates: Vec<MemoryEntry>,
        budget: &EnhancementBudget,
    ) -> common::Result<Vec<MemoryEntry>>;
}
