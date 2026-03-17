//! Concrete InsightEmbedder — wraps EmbeddingEngine + VectorStore.
//!
//! Follows the same adapter pattern as `NoteEmbeddingAdapter` in the `agent` crate.

use async_trait::async_trait;
use chrono::Utc;
use feature_insights::InsightEmbedder;
use std::sync::Arc;
use tools::embedding_engine::EmbeddingEngine;
use tracing::debug;

pub struct InsightEmbedderImpl {
    engine: Arc<EmbeddingEngine>,
    store: storage::VectorStore,
}

impl InsightEmbedderImpl {
    pub fn new(engine: Arc<EmbeddingEngine>, store: storage::VectorStore) -> Self {
        Self { engine, store }
    }
}

#[async_trait]
impl InsightEmbedder for InsightEmbedderImpl {
    async fn embed_and_store(&self, insight_id: &str, content: &str) -> Result<(), String> {
        // Truncate to ~2000 bytes for embedding (UTF-8 safe)
        let text = common::truncate_at_boundary(content, 2000);

        // embed_async takes Arc<Self> as receiver: engine.clone().embed_async(text).await
        let vector = self
            .engine
            .clone()
            .embed_async(text.to_string())
            .await
            .map_err(|e| format!("embedding failed: {e}"))?;

        // upsert_embedding expects &[(&str, &str)] for extra fields
        let updated_at = Utc::now().to_rfc3339();
        let extra_fields: &[(&str, &str)] = &[("updated_at", &updated_at)];

        self.store
            .upsert_embedding("insight_embeddings", insight_id, &vector, extra_fields)
            .await
            .map_err(|e| format!("upsert failed: {e}"))?;

        debug!(insight_id, "embedded insight content");
        Ok(())
    }

    async fn similarity(&self, _id_a: &str, _id_b: &str) -> Option<f64> {
        // Phase 2 placeholder — semantic drift calculation requires fetching
        // raw vectors from LanceDB, which needs a new VectorStore method.
        // Returns None → semantic_drift defaults to 0.0 (no drift detected).
        // Phase 3 adds the full implementation with vector fetch + cosine_similarity.
        None
    }
}
