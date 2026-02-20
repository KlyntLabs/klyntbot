//! Embedding handler trait for dependency inversion.
//!
//! Allows the TodoTool to request embeddings without depending on fastembed
//! or the agent's embedding engine directly.

use async_trait::async_trait;
use common::Result;

use crate::types::Todo;

/// Trait for embedding handlers.
/// Implemented by EmbeddingEngine in the agent crate (Layer 5).
#[async_trait]
pub trait EmbeddingHandler: Send + Sync {
    /// Generate and store an embedding for a todo item (best-effort).
    async fn embed_todo(&self, todo: &Todo) -> Result<()>;

    /// Embed a query string and return the embedding vector.
    /// Used for semantic search.
    async fn embed_query(&self, query: &str) -> Result<Vec<f32>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct NoOpEmbedder;

    #[async_trait]
    impl EmbeddingHandler for NoOpEmbedder {
        async fn embed_todo(&self, _todo: &Todo) -> Result<()> {
            Ok(())
        }

        async fn embed_query(&self, _query: &str) -> Result<Vec<f32>> {
            Ok(vec![0.0f32; 384])
        }
    }

    #[tokio::test]
    async fn test_trait_is_object_safe() {
        let handler: Arc<dyn EmbeddingHandler> = Arc::new(NoOpEmbedder);
        let todo = Todo::default_instance();
        handler.embed_todo(&todo).await.unwrap();
        let vec = handler.embed_query("test").await.unwrap();
        assert_eq!(vec.len(), 384);
    }
}
