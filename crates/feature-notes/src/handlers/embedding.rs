//! Embedding handler trait for note semantic search.
//!
//! Follows the dependency inversion pattern from feature-tasks:
//! the trait is defined here (L4), implemented in agent crate (L5).

use async_trait::async_trait;
use common::Result;

use crate::models::NoteRow;

/// Trait for note embedding operations.
/// Implemented by NoteEmbeddingAdapter in the agent crate.
#[async_trait]
pub trait NoteEmbeddingHandler: Send + Sync {
    /// Generate and store an embedding for a note (best-effort, fire-and-forget).
    async fn embed_note(&self, note: &NoteRow) -> Result<()>;

    /// Embed a query string and return the raw embedding vector.
    async fn embed_query(&self, query: &str) -> Result<Vec<f32>>;

    /// Search for notes similar to the given query vector.
    /// Returns (note_id, similarity_score) pairs.
    async fn search_similar(
        &self,
        query_vec: &[f32],
        limit: usize,
        min_score: f64,
    ) -> Result<Vec<(String, f64)>>;
}
