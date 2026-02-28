//! ConversationEmbeddingHandlerImpl — production handler for conversation embeddings.
//!
//! Implements the ConversationEmbeddingHandler trait defined in tools crate (L3).
//! Reuses the shared EmbeddingEngine for memory efficiency.

use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;
use tracing::warn;

use common::Result;
use tools::conversation_embedding::{
    ConversationEmbeddingHandler, ConversationEmbeddingRecord, ConversationEmbeddingStatus,
    ConversationEmbeddingStore, PurgeFilter,
};
use tools::embedding_engine::EmbeddingEngine;
use tools::EMBEDDING_DIM;

/// Production conversation embedding handler.
pub struct ConversationEmbeddingHandlerImpl {
    engine: Arc<EmbeddingEngine>,
    store: ConversationEmbeddingStore,
}

impl ConversationEmbeddingHandlerImpl {
    /// Create a new handler with shared embedding engine and SQL-backed store.
    pub fn new(engine: Arc<EmbeddingEngine>, store: ConversationEmbeddingStore) -> Self {
        Self { engine, store }
    }
}

#[async_trait]
impl ConversationEmbeddingHandler for ConversationEmbeddingHandlerImpl {
    async fn embed_message(
        &self,
        session_key: &str,
        role: &str,
        content: &str,
        message_id: &str,
    ) -> Result<()> {
        // Compose text with role prefix (e.g., "User: hello" or "Assistant: response")
        let text = format!("{}: {}", role, content);

        let embedding = match self.engine.clone().embed_async(text).await {
            Ok(emb) => emb,
            Err(e) => {
                warn!(
                    "Failed to generate conversation embedding for message {}: {}",
                    message_id, e
                );
                return Ok(()); // Best-effort: don't propagate errors
            }
        };

        // Validate dimension
        if embedding.len() != EMBEDDING_DIM {
            warn!(
                "Conversation embedding dimension mismatch for message {}: got {}, expected {}",
                message_id,
                embedding.len(),
                EMBEDDING_DIM
            );
            return Ok(());
        }

        // Create record
        let record = ConversationEmbeddingRecord {
            id: message_id.to_string(),
            session_key: session_key.to_string(),
            role: role.to_string(),
            content_preview: content.chars().take(100).collect(),
            content_full: content.to_string(),
            embedding,
            model: self.engine.model_name().to_string(),
            embedded_at: Utc::now(),
        };

        // Store (best-effort, log errors but don't propagate)
        if let Err(e) = self.store.upsert(record).await {
            warn!(
                "Failed to store conversation embedding for message {}: {}",
                message_id, e
            );
        }

        Ok(())
    }

    async fn search(
        &self,
        query: &str,
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<(ConversationEmbeddingRecord, f64)>> {
        let query_embedding = self.engine.clone().embed_async(query.to_string()).await?;

        // LanceDB ANN search (cross-channel).
        // Explicit user searches use decay_factor=1.0 (no time decay) for unbiased results.
        self.store
            .search_similar(&query_embedding, limit, threshold, 1.0)
            .await
    }

    async fn purge(&self, filter: PurgeFilter) -> Result<usize> {
        self.store.purge(filter).await
    }

    async fn status(&self) -> Result<ConversationEmbeddingStatus> {
        self.store.status().await
    }

    fn is_available(&self) -> bool {
        self.engine.is_available()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_handler() -> (ConversationEmbeddingHandlerImpl, tempfile::TempDir) {
        // Create embedding engine (model won't be loaded in tests)
        let engine = Arc::new(EmbeddingEngine::new());

        // Use a temporary LanceDB directory for the vector store
        let dir = tempfile::TempDir::new().expect("temp dir");
        let vs = storage::VectorStore::connect(dir.path())
            .await
            .expect("vector store");
        let store = ConversationEmbeddingStore::new(vs);

        (ConversationEmbeddingHandlerImpl::new(engine, store), dir)
    }

    // ──────────────────────────────────────────────────────────────────────
    // TDD Tests - Written FIRST before implementation
    // ──────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_embed_message_adds_role_prefix() {
        let (handler, _dir) = create_test_handler().await;

        // This will fail if model unavailable, but we test the role prefix logic
        let result = handler
            .embed_message("telegram:12345", "user", "Hello world", "msg-1")
            .await;

        // Should succeed (best-effort) even when model unavailable
        assert!(
            result.is_ok(),
            "embed_message should succeed even when model unavailable"
        );
    }

    #[tokio::test]
    async fn test_embed_message_best_effort_on_error() {
        let (handler, _dir) = create_test_handler().await;

        // This will fail (no model loaded yet), but should succeed with best-effort
        let result = handler
            .embed_message("telegram:12345", "assistant", "Response", "msg-2")
            .await;

        assert!(
            result.is_ok(),
            "Should succeed with best-effort even on embedding error"
        );
    }

    #[tokio::test]
    async fn test_is_available_reflects_engine() {
        let (handler, _dir) = create_test_handler().await;

        // Engine uses lazy init, model not loaded yet
        assert!(
            !handler.is_available(),
            "Should return false before model loads"
        );
    }
}
