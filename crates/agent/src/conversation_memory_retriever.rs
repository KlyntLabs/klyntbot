//! ConversationMemoryRetriever — implements MemoryRetriever for automatic contextual recall.
//!
//! Uses EmbeddingEngine (fastembed) + ConversationEmbeddingStore (pgvector ANN)
//! to retrieve relevant past conversation snippets during context assembly.
//! Cross-channel: searches all sessions, not scoped to current.

use std::sync::Arc;

use async_trait::async_trait;
use tracing::warn;

use context_engine::memory_retriever::{MemoryEntry, MemoryRetriever};
use tools::conversation_embedding::ConversationEmbeddingStore;
use tools::embedding_engine::EmbeddingEngine;

/// Retrieves relevant conversation memories using pgvector ANN search.
///
/// Injected into `ContextEngine` via `.with_memory_retriever()` at startup.
/// Called automatically on every message during context assembly.
pub struct ConversationMemoryRetriever {
    engine: Arc<EmbeddingEngine>,
    store: ConversationEmbeddingStore,
    threshold: f64,
}

impl ConversationMemoryRetriever {
    /// Create a new retriever.
    ///
    /// - `engine`: shared embedding engine (fastembed, lazy-loaded)
    /// - `store`: pgvector-backed conversation embedding store
    /// - `threshold`: minimum cosine similarity for retrieval (0.0-1.0)
    pub fn new(
        engine: Arc<EmbeddingEngine>,
        store: ConversationEmbeddingStore,
        threshold: f64,
    ) -> Self {
        Self { engine, store, threshold }
    }
}

#[async_trait]
impl MemoryRetriever for ConversationMemoryRetriever {
    async fn retrieve(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        // 1. Embed query (CPU-bound, use blocking thread pool)
        let embedding = {
            let engine = Arc::clone(&self.engine);
            let q = query.to_string();
            match tokio::task::spawn_blocking(move || engine.embed(&q)).await {
                Ok(Ok(emb)) => emb,
                Ok(Err(e)) => {
                    warn!("MemoryRetriever: embedding failed: {}", e);
                    return Vec::new();
                }
                Err(e) => {
                    warn!("MemoryRetriever: spawn_blocking failed: {}", e);
                    return Vec::new();
                }
            }
        };

        // 2. pgvector ANN search (cross-channel)
        match self.store.search_similar(&embedding, limit, self.threshold).await {
            Ok(results) => results
                .into_iter()
                .map(|(record, score)| MemoryEntry {
                    id: record.id,
                    content: record.content_preview,
                    score,
                })
                .collect(),
            Err(e) => {
                warn!("MemoryRetriever: pgvector search failed: {}", e);
                Vec::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_retriever() -> ConversationMemoryRetriever {
        let engine = Arc::new(EmbeddingEngine::new());
        let pool = storage::StoragePool::connect_lazy("postgres://localhost/klyntbot_test")
            .expect("test pool");
        let repo = storage::ConvEmbeddingRepo::new(pool.inner().clone());
        let store = ConversationEmbeddingStore::new(repo);
        ConversationMemoryRetriever::new(engine, store, 0.5)
    }

    #[tokio::test]
    async fn test_retrieve_returns_empty_when_engine_unavailable() {
        let retriever = make_retriever();
        let results = retriever.retrieve("test query", 5).await;
        assert!(results.is_empty(), "Should return empty when model unavailable");
    }

    #[tokio::test]
    async fn test_retrieve_empty_query() {
        let retriever = make_retriever();
        let results = retriever.retrieve("", 5).await;
        assert!(results.is_empty(), "Empty query should return empty results");
    }

    #[tokio::test]
    async fn test_retrieve_zero_limit() {
        let retriever = make_retriever();
        let results = retriever.retrieve("test", 0).await;
        assert!(results.is_empty(), "Zero limit should return empty results");
    }

    #[test]
    fn test_retriever_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ConversationMemoryRetriever>();
    }
}
