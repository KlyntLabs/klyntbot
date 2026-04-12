use std::sync::Arc;

use async_trait::async_trait;
use context_engine::memory_retriever::MemoryRetriever;
use context_engine::MemoryScorer;

/// Wraps a `MemoryRetriever` to implement `MemoryScorer` for tiered compression.
///
/// Uses the existing retrieval infrastructure to score text passages for
/// cognitive relevance. Retrieves top-1 result for each text and uses
/// its score as the relevance signal.
pub struct CognitiveMemoryScorer {
    retriever: Arc<dyn MemoryRetriever>,
}

impl CognitiveMemoryScorer {
    pub fn new(retriever: Arc<dyn MemoryRetriever>) -> Self {
        Self { retriever }
    }
}

#[async_trait]
impl MemoryScorer for CognitiveMemoryScorer {
    async fn score_batch(&self, texts: &[String]) -> Vec<f64> {
        let futures: Vec<_> = texts
            .iter()
            .map(|text| {
                let retriever = Arc::clone(&self.retriever);
                let text = text.clone();
                async move {
                    let entries = retriever.retrieve(&text, 1).await;
                    entries.first().map(|e| e.score).unwrap_or(0.0)
                }
            })
            .collect();
        futures_util::future::join_all(futures).await
    }
}
