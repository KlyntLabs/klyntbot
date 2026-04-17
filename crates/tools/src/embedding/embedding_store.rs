//! In-memory embedding index.
//!
//! `EmbeddingRecord` is still used by `EmbeddingHandler::embed_todo()` as a return type.
//! The `EmbeddingStore` itself is a lightweight in-memory cache; persistence is now
//! handled directly by `storage::VectorStore` (LanceDB).

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use common::Result;

/// A single embedding record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRecord {
    pub id: String,
    pub embedding: Vec<f32>,
    pub model: String,
    pub embedded_at: Timestamp,
}

/// In-memory embedding index.
///
/// Used as a lightweight cache; LanceDB persistence goes through `storage::VectorStore`.
pub struct EmbeddingStore {
    index: HashMap<String, EmbeddingRecord>,
}

impl EmbeddingStore {
    pub fn new() -> Self {
        Self {
            index: HashMap::new(),
        }
    }

    /// Upsert an embedding record into the in-memory index.
    pub async fn upsert(&mut self, record: EmbeddingRecord) -> Result<()> {
        self.index.insert(record.id.clone(), record);
        Ok(())
    }

    /// Delete an embedding by ID.
    pub async fn delete(&mut self, id: &str) -> Result<()> {
        self.index.remove(id);
        Ok(())
    }

    /// Get a single embedding record by ID.
    pub async fn get(&self, id: &str) -> Result<Option<&EmbeddingRecord>> {
        Ok(self.index.get(id))
    }

    /// Get all embedding records.
    pub async fn get_all(&self) -> Result<&HashMap<String, EmbeddingRecord>> {
        Ok(&self.index)
    }

    /// Find IDs that don't have embeddings in the in-memory index.
    pub fn ids_missing_embeddings(&self, todo_ids: &[String]) -> Vec<String> {
        todo_ids
            .iter()
            .filter(|id| !self.index.contains_key(id.as_str()))
            .cloned()
            .collect()
    }
}

impl Default for EmbeddingStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EMBEDDING_DIM;

    fn create_test_record(id: &str) -> EmbeddingRecord {
        EmbeddingRecord {
            id: id.to_string(),
            embedding: vec![0.1; EMBEDDING_DIM],
            model: "test-model".to_string(),
            embedded_at: Timestamp::now(),
        }
    }

    #[tokio::test]
    async fn test_ids_missing_embeddings_with_index() {
        let mut store = EmbeddingStore::new();

        store.upsert(create_test_record("id-1")).await.unwrap();
        store.upsert(create_test_record("id-3")).await.unwrap();

        let all_ids = vec![
            "id-1".to_string(),
            "id-2".to_string(),
            "id-3".to_string(),
            "id-4".to_string(),
        ];
        let missing = store.ids_missing_embeddings(&all_ids);
        assert_eq!(missing, vec!["id-2".to_string(), "id-4".to_string()]);
    }

    #[tokio::test]
    async fn test_ids_missing_embeddings_empty_store() {
        let store = EmbeddingStore::new();

        let all_ids = vec!["a".to_string(), "b".to_string()];
        let missing = store.ids_missing_embeddings(&all_ids);
        assert_eq!(missing.len(), 2);
    }
}
