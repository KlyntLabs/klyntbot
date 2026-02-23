//! TodoEmbeddingHandlerImpl — implements feature_todo::EmbeddingHandler.
//!
//! Wraps the shared EmbeddingEngine (fastembed) and a storage::VectorStore.
//! Satisfies feature-todo's dependency inversion contract for embedding operations.

use async_trait::async_trait;
use common::Result;
use feature_todo::{EmbeddingHandler, Todo};
use std::sync::Arc;
use tools::embedding_engine::EmbeddingEngine;
use tracing::debug;

/// Production implementation of feature_todo::EmbeddingHandler.
///
/// Uses the shared `EmbeddingEngine` for vector generation and
/// `storage::VectorStore` for LanceDB persistence.
pub struct TodoEmbeddingHandlerImpl {
    engine: Arc<EmbeddingEngine>,
    store: storage::VectorStore,
}

impl TodoEmbeddingHandlerImpl {
    pub fn new(engine: Arc<EmbeddingEngine>, store: storage::VectorStore) -> Self {
        Self { engine, store }
    }

    /// Compose searchable text for a todo: "{title} {description} {tags}".
    fn compose_text(todo: &Todo) -> String {
        format!(
            "{} {} {}",
            todo.title,
            todo.description.as_deref().unwrap_or(""),
            todo.tags.join(" ")
        )
    }
}

#[async_trait]
impl EmbeddingHandler for TodoEmbeddingHandlerImpl {
    async fn embed_todo(&self, todo: &Todo) -> Result<()> {
        let text = Self::compose_text(todo);
        let todo_id = todo.id.clone();
        let engine = self.engine.clone();

        debug!(todo_id = %todo_id, "Generating embedding for todo");

        // CPU-bound — run in blocking thread pool
        let embedding = tokio::task::spawn_blocking(move || engine.embed(&text))
            .await
            .map_err(|e| {
                common::ToolError::ExecutionFailed(format!("Embedding task panicked: {}", e))
            })??;

        let model_name = "paraphrase-multilingual-MiniLM-L12-v2";
        self.store
            .upsert_embedding(
                "todo_embeddings",
                &todo_id,
                &embedding,
                &[("model", model_name)],
            )
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;

        Ok(())
    }

    async fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        let query = query.to_string();
        let engine = self.engine.clone();

        debug!(query_len = query.len(), "Generating query embedding");

        let embedding = tokio::task::spawn_blocking(move || engine.embed(&query))
            .await
            .map_err(|e| {
                common::ToolError::ExecutionFailed(format!("Embedding task panicked: {}", e))
            })??;

        Ok(embedding)
    }
}
