//! Adapter bridging `VectorStore` + `TextEmbedder` → `TreeNodeEmbeddingSearch`.
//!
//! Dependency inversion: `NoteTreeNavigator` (L3 context_engine) defines
//! `TreeNodeEmbeddingSearch`; this adapter (L5 agent) implements it using
//! concrete storage + cognitive types.

use std::sync::Arc;

use async_trait::async_trait;

use cognitive::TextEmbedder;
use context_engine::insight_forge::note_tree_navigator::{TreeNodeEmbeddingSearch, TreeNodeHit};

pub struct TreeNodeSearchAdapter {
    vector_store: Arc<storage::VectorStore>,
    embedder: Arc<dyn TextEmbedder>,
}

impl TreeNodeSearchAdapter {
    pub fn new(vector_store: Arc<storage::VectorStore>, embedder: Arc<dyn TextEmbedder>) -> Self {
        Self {
            vector_store,
            embedder,
        }
    }
}

#[async_trait]
impl TreeNodeEmbeddingSearch for TreeNodeSearchAdapter {
    async fn search_tree_nodes(
        &self,
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f64,
        note_id_filter: Option<&str>,
    ) -> common::Result<Vec<TreeNodeHit>> {
        let results = self
            .vector_store
            .search_tree_node_embeddings(query_embedding, limit, min_similarity, note_id_filter)
            .await?;

        Ok(results
            .into_iter()
            .map(|r| TreeNodeHit {
                node_id: r.node_id,
                note_id: r.note_id,
                similarity: r.score,
            })
            .collect())
    }

    async fn embed_query(&self, query: &str) -> common::Result<Vec<f32>> {
        self.embedder.embed(query).await
    }
}
