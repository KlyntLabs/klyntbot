//! SQL-backed embedding storage for semantic search (pgvector).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use common::Result;
use storage::EmbeddingRepo;

/// A single embedding record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRecord {
    pub id: String,
    pub embedding: Vec<f32>,
    pub model: String,
    pub embedded_at: DateTime<Utc>,
}

/// In-memory embedding index backed by SQL (pgvector).
pub struct EmbeddingStore {
    index: HashMap<String, EmbeddingRecord>,
    sql_repo: EmbeddingRepo,
}

impl EmbeddingStore {
    /// Create a store backed by SQL (pgvector) via `EmbeddingRepo`.
    /// The in-memory index is kept in sync for `get_all` and `ids_missing_embeddings`.
    pub fn from_repo(repo: EmbeddingRepo) -> Self {
        Self {
            index: HashMap::new(),
            sql_repo: repo,
        }
    }

    /// Upsert an embedding record (SQL + update in-memory index).
    pub async fn upsert(&mut self, record: EmbeddingRecord) -> Result<()> {
        let vector = pgvector::Vector::from(record.embedding.clone());
        self.sql_repo
            .upsert(&record.id, &vector, &record.model)
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;
        self.index.insert(record.id.clone(), record);
        Ok(())
    }

    /// Delete an embedding by ID.
    pub async fn delete(&mut self, id: &str) -> Result<()> {
        self.sql_repo
            .delete(id)
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;
        self.index.remove(id);
        Ok(())
    }

    /// Get a single embedding record by ID.
    /// Fetches from SQL if not in the in-memory cache.
    pub async fn get(&mut self, id: &str) -> Result<Option<&EmbeddingRecord>> {
        // Check in-memory cache first
        if self.index.contains_key(id) {
            return Ok(self.index.get(id));
        }
        // Fetch from SQL
        match self.sql_repo.get(id).await {
            Ok(row) => {
                let record = EmbeddingRecord {
                    id: row.todo_id,
                    embedding: row.embedding.as_slice().to_vec(),
                    model: row.model,
                    embedded_at: row.updated_at,
                };
                self.index.insert(record.id.clone(), record);
                Ok(self.index.get(id))
            }
            Err(storage::StorageError::NotFound(_)) => Ok(None),
            Err(e) => Err(common::ToolError::ExecutionFailed(e.to_string()).into()),
        }
    }

    /// Get all embedding records (for brute-force search).
    pub async fn get_all(&mut self) -> Result<&HashMap<String, EmbeddingRecord>> {
        Ok(&self.index)
    }

    /// Find todo IDs that don't have embeddings in the store.
    pub fn ids_missing_embeddings(&self, todo_ids: &[String]) -> Vec<String> {
        todo_ids
            .iter()
            .filter(|id| !self.index.contains_key(id.as_str()))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EMBEDDING_DIM;

    // Note: SQL-backed tests require a running PostgreSQL instance.
    // Unit tests for EmbeddingRecord and ids_missing_embeddings
    // can run without a database.

    fn test_repo() -> EmbeddingRepo {
        let pool =
            storage::StoragePool::connect_lazy("postgres://localhost/klyntbot_test").unwrap();
        EmbeddingRepo::new(pool.inner().clone())
    }

    fn create_test_record(id: &str) -> EmbeddingRecord {
        EmbeddingRecord {
            id: id.to_string(),
            embedding: vec![0.1; EMBEDDING_DIM],
            model: "test-model".to_string(),
            embedded_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_ids_missing_embeddings_with_index() {
        let mut store = EmbeddingStore::from_repo(test_repo());

        // Manually populate the in-memory index
        store
            .index
            .insert("id-1".to_string(), create_test_record("id-1"));
        store
            .index
            .insert("id-3".to_string(), create_test_record("id-3"));

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
        let store = EmbeddingStore::from_repo(test_repo());

        let all_ids = vec!["a".to_string(), "b".to_string()];
        let missing = store.ids_missing_embeddings(&all_ids);
        assert_eq!(missing.len(), 2);
    }
}
