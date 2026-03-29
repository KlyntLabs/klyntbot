//! Entity embedding operations for entity similarity search.

use crate::error::StorageError;

use super::VectorStore;

impl VectorStore {
    /// Upsert an entity embedding.
    pub async fn upsert_entity_embedding(
        &self,
        id: &str,
        embedding: &[f32],
        name: &str,
        entity_type: &str,
        description: &str,
    ) -> Result<(), StorageError> {
        self.upsert_embedding(
            "entity_embeddings",
            id,
            embedding,
            &[
                ("name", name),
                ("entity_type", entity_type),
                ("description", description),
            ],
        )
        .await
    }
}
