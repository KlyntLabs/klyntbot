//! EntityEmbedder — backfills and upserts entity embeddings into LanceDB.
//!
//! Reads from the `entities` SQLite table and writes to the `entity_embeddings`
//! LanceDB table so that entity similarity search is available for RAG.

use std::sync::Arc;

use cognitive::TextEmbedder;
use tracing::{debug, warn};

/// Embeds entities into the `entity_embeddings` LanceDB table.
pub struct EntityEmbedder {
    vector_store: Arc<storage::VectorStore>,
    embedder: Arc<dyn TextEmbedder>,
}

impl EntityEmbedder {
    pub fn new(vector_store: Arc<storage::VectorStore>, embedder: Arc<dyn TextEmbedder>) -> Self {
        Self {
            vector_store,
            embedder,
        }
    }

    /// Embed a single entity and upsert it into LanceDB.
    pub async fn embed_entity(
        &self,
        id: &str,
        name: &str,
        description: Option<&str>,
        entity_type: &str,
    ) -> common::Result<()> {
        let text = match description {
            Some(desc) if !desc.is_empty() => format!("{name}: {desc}"),
            _ => name.to_string(),
        };
        let embedding = self.embedder.embed(&text).await?;
        self.vector_store
            .upsert_entity_embedding(id, &embedding, name, entity_type, description.unwrap_or(""))
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;
        debug!(entity_id = %id, entity_type = %entity_type, "Entity embedding upserted");
        Ok(())
    }

    /// Backfill embeddings for all entities.
    ///
    /// Entities are embedded by name (+ description if available).
    /// Returns the number of entities successfully embedded.
    pub async fn backfill_all(&self, pool: &sqlx::SqlitePool) -> common::Result<usize> {
        let entities: Vec<(String, String, Option<String>, String)> = sqlx::query_as(
            "SELECT id, name, description, entity_type FROM entities",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| common::ToolError::ExecutionFailed(format!("entity backfill query: {e}")))?;

        let total = entities.len();
        let mut count = 0usize;

        for (id, name, desc, etype) in &entities {
            if self
                .embed_entity(id, name, desc.as_deref(), etype)
                .await
                .is_ok()
            {
                count += 1;
            } else {
                warn!(entity_id = %id, "EntityEmbedder: failed to embed entity (skipped)");
            }
        }

        debug!(
            embedded = count,
            total = total,
            "EntityEmbedder: backfill complete"
        );
        Ok(count)
    }
}
