//! NoteEmbeddingAdapter — implements feature_notes::handlers::embedding::NoteEmbeddingHandler.
//!
//! Wraps the shared EmbeddingEngine and storage::VectorStore.
//! Follows the same pattern as TaskEmbeddingAdapter.

use async_trait::async_trait;
use common::Result;
use feature_notes::handlers::embedding::NoteEmbeddingHandler;
use feature_notes::models::NoteRow;
use std::sync::Arc;
use tools::embedding_engine::EmbeddingEngine;
use tracing::debug;

pub struct NoteEmbeddingAdapter {
    engine: Arc<EmbeddingEngine>,
    store: storage::VectorStore,
}

impl NoteEmbeddingAdapter {
    pub fn new(engine: Arc<EmbeddingEngine>, store: storage::VectorStore) -> Self {
        Self { engine, store }
    }

    /// Compose searchable text: "{title} {body_preview}".
    /// Truncate body to first 500 chars to keep embedding focused.
    fn compose_text(note: &NoteRow) -> String {
        let body = &note.body;
        let truncated = if body.len() > 500 {
            &body[..500]
        } else {
            body.as_str()
        };
        format!("{} {}", note.title, truncated)
    }
}

#[async_trait]
impl NoteEmbeddingHandler for NoteEmbeddingAdapter {
    async fn embed_note(&self, note: &NoteRow) -> Result<()> {
        let text = Self::compose_text(note);
        let note_id = note.id.clone();

        debug!(note_id = %note_id, "Generating embedding for note");

        let embedding = self.engine.clone().embed_async(text).await?;

        let model_name = "paraphrase-multilingual-MiniLM-L12-v2";
        self.store
            .upsert_embedding(
                "note_embeddings",
                &note_id,
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
            "Generating query embedding for notes"
        );
        self.engine.clone().embed_async(query.to_string()).await
    }

    async fn search_similar(
        &self,
        query_vec: &[f32],
        limit: usize,
        min_score: f64,
    ) -> Result<Vec<(String, f64)>> {
        self.store
            .search_similar("note_embeddings", query_vec, limit, min_score)
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()).into())
    }
}
