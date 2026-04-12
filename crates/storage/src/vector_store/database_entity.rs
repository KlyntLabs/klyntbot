//! Database entity embedding operations for flexible database entities.

use crate::error::StorageError;

use super::VectorStore;

impl VectorStore {
    /// Upsert an embedding for a database entity (from the flexible database engine).
    pub async fn upsert_database_entity_embedding(
        &self,
        table_name: &str,
        entity_id: &str,
        embedding: &[f32],
        database_id: &str,
        field_context: &str,
    ) -> Result<(), StorageError> {
        self.upsert_embedding(
            table_name,
            entity_id,
            embedding,
            &[
                ("database_id", database_id),
                ("field_context", field_context),
            ],
        )
        .await
    }
}
