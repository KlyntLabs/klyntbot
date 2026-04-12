use async_trait::async_trait;

/// Scores text passages for cognitive relevance (0.0–1.0).
///
/// Implemented in the `agent` crate by wrapping `UnifiedMemoryService`.
/// Lives here (L3) to avoid `context_engine` depending on `cognitive` (L5).
#[async_trait]
pub trait MemoryScorer: Send + Sync {
    /// Score relevance of text passages. Returns one score per input.
    async fn score_batch(&self, texts: &[String]) -> Vec<f64>;
}
