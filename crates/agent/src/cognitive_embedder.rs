//! Production implementation of `SemanticFactEmbedder`.
//!
//! Wraps `EmbeddingEngine` (for vector generation) and `VectorStore`
//! (for LanceDB persistence). Constructed in `AgentLoopBuilder` and
//! injected into `CognitiveContextSource`.

use std::sync::Arc;

use async_trait::async_trait;
use tracing::{debug, warn};

use cognitive::embedder::SemanticFactEmbedder;
use cognitive::types::SemanticFact;
use tools::EmbeddingEngine;

/// Production embedder for cognitive semantic facts.
///
/// Generates 384-dim embeddings from SPO triple text using fastembed,
/// stores/removes vectors in LanceDB, and searches by cosine similarity
/// with domain pre-filtering.
pub struct SemanticFactEmbedderImpl {
    engine: Arc<EmbeddingEngine>,
    store: storage::VectorStore,
}

impl SemanticFactEmbedderImpl {
    pub fn new(engine: Arc<EmbeddingEngine>, store: storage::VectorStore) -> Self {
        Self { engine, store }
    }

    /// Compose the text to embed from an SPO triple.
    fn fact_text(fact: &SemanticFact) -> String {
        format!("{} {} {}", fact.subject, fact.predicate, fact.object)
    }
}

#[async_trait]
impl SemanticFactEmbedder for SemanticFactEmbedderImpl {
    async fn embed_and_store_fact(&self, fact: &SemanticFact) -> common::Result<()> {
        let text = Self::fact_text(fact);
        let embedding = self.engine.clone().embed_async(text.clone()).await?;

        self.store
            .upsert_cognitive_fact(
                &fact.id,
                &embedding,
                &fact.domain,
                &text,
                (fact.confidence * fact.stability) as f32,
                fact.stability as f32,
                fact.confidence as f32,
            )
            .await?;

        debug!(fact_id = %fact.id, "Embedded cognitive fact");
        Ok(())
    }

    async fn remove_embedding(&self, fact_id: &str) -> common::Result<()> {
        self.store
            .delete("cognitive_fact_embeddings", fact_id)
            .await?;
        debug!(fact_id = %fact_id, "Removed cognitive fact embedding");
        Ok(())
    }

    async fn search_similar(
        &self,
        query: &str,
        domains: &[&str],
        top_k: usize,
        min_similarity: f64,
    ) -> common::Result<Vec<(String, f64)>> {
        let query_embedding = self.engine.clone().embed_async(query.to_string()).await?;

        let results = self
            .store
            .search_cognitive_facts(&query_embedding, domains, top_k, min_similarity)
            .await?;

        Ok(results)
    }

    async fn reindex_all(&self, facts: &[SemanticFact]) -> common::Result<usize> {
        let mut count = 0;
        for fact in facts {
            match self.embed_and_store_fact(fact).await {
                Ok(()) => count += 1,
                Err(e) => warn!(fact_id = %fact.id, "Failed to reindex fact: {e}"),
            }
        }
        debug!("Reindexed {count}/{} cognitive facts", facts.len());
        Ok(count)
    }

    fn is_available(&self) -> bool {
        self.engine.is_available()
    }
}
