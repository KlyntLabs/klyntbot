//! TaskEmbeddingAdapter — implements feature_tasks::EmbeddingHandler.
//!
//! Wraps the shared EmbeddingEngine (fastembed) and a storage::VectorStore.
//! Satisfies feature-tasks' dependency inversion contract for embedding operations.
//! Mirrors TodoEmbeddingHandlerImpl but adapted for feature_tasks::Task.

use async_trait::async_trait;
use common::Result;
use feature_tasks::{EmbeddingHandler, Task};
use std::sync::Arc;
use tools::embedding_engine::EmbeddingEngine;
use tracing::debug;

/// Production implementation of feature_tasks::EmbeddingHandler.
///
/// Uses the shared `EmbeddingEngine` for vector generation and
/// `storage::VectorStore` for LanceDB persistence.
pub struct TaskEmbeddingAdapter {
    engine: Arc<EmbeddingEngine>,
    store: storage::VectorStore,
}

impl TaskEmbeddingAdapter {
    pub fn new(engine: Arc<EmbeddingEngine>, store: storage::VectorStore) -> Self {
        Self { engine, store }
    }

    /// Compose searchable text for a task: "{title} {description} {tags}".
    fn compose_text(task: &Task) -> String {
        format!(
            "{} {} {}",
            task.title,
            task.description.as_deref().unwrap_or(""),
            task.tags.join(" ")
        )
    }
}

#[async_trait]
impl EmbeddingHandler for TaskEmbeddingAdapter {
    async fn embed_task(&self, task: &Task) -> Result<()> {
        let text = Self::compose_text(task);
        let task_id = task.id.clone();

        debug!(task_id = %task_id, "Generating embedding for task");

        let embedding = self.engine.clone().embed_async(text).await?;

        let model_name = "paraphrase-multilingual-MiniLM-L12-v2";
        self.store
            .upsert_embedding(
                "task_embeddings",
                &task_id,
                &embedding,
                &[("model", model_name)],
            )
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;

        Ok(())
    }

    async fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        debug!(
            query_len = query.len(),
            "Generating query embedding for tasks"
        );

        self.engine.clone().embed_async(query.to_string()).await
    }
}
