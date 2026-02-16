//! JSONL-based embedding storage for semantic search.
//!
//! Follows the same append-only journal pattern as `TodoStore`:
//! - Upserts and deletes are appended as journal entries
//! - On load, entries are replayed to reconstruct the in-memory index
//! - Compaction rewrites the file with only live records
//! - Corrupted lines are skipped with a warning (graceful degradation)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tracing::warn;

use crate::embedding_engine::EMBEDDING_DIM;
use common::Result;

/// A single embedding record persisted to the JSONL store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRecord {
    pub id: String,
    pub embedding: Vec<f32>,
    pub model: String,
    pub embedded_at: DateTime<Utc>,
}

/// Journal entry for append-only JSONL storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "_op")]
enum EmbeddingJournalEntry {
    #[serde(rename = "upsert")]
    Upsert { record: EmbeddingRecord },
    #[serde(rename = "delete")]
    Delete { id: String },
}

/// In-memory embedding index backed by a JSONL file.
pub struct EmbeddingStore {
    file_path: PathBuf,
    index: HashMap<String, EmbeddingRecord>,
    loaded: bool,
    journal_len: usize,
}

impl EmbeddingStore {
    /// Create a new store pointing to the given JSONL file.
    /// The file is NOT loaded until `load()` is called (lazy).
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            index: HashMap::new(),
            loaded: false,
            journal_len: 0,
        }
    }

    /// Get the file path for this store.
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    /// Load (replay) the JSONL journal from disk into the in-memory index.
    /// Corrupted or dimension-mismatched lines are skipped with a warning.
    pub async fn load(&mut self) -> Result<()> {
        self.index.clear();
        self.journal_len = 0;

        if !self.file_path.exists() {
            self.loaded = true;
            return Ok(());
        }

        let content = tokio::fs::read_to_string(&self.file_path)
            .await
            .map_err(|e| {
                common::ToolError::ExecutionFailed(format!("Failed to read embeddings file: {}", e))
            })?;

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            self.journal_len += 1;

            match serde_json::from_str::<EmbeddingJournalEntry>(line) {
                Ok(EmbeddingJournalEntry::Upsert { record }) => {
                    // Validate dimension
                    if record.embedding.len() != EMBEDDING_DIM {
                        warn!(
                            "Skipping embedding for {} at line {}: dimension {} != expected {}",
                            record.id,
                            line_num + 1,
                            record.embedding.len(),
                            EMBEDDING_DIM
                        );
                        continue;
                    }
                    self.index.insert(record.id.clone(), record);
                }
                Ok(EmbeddingJournalEntry::Delete { id }) => {
                    self.index.remove(&id);
                }
                Err(e) => {
                    warn!(
                        "Skipping corrupted embedding line {} in {:?}: {}",
                        line_num + 1,
                        self.file_path,
                        e
                    );
                }
            }
        }

        self.loaded = true;
        Ok(())
    }

    /// Ensure the store is loaded before any read/write operation.
    async fn ensure_loaded(&mut self) -> Result<()> {
        if !self.loaded {
            self.load().await?;
        }
        Ok(())
    }

    /// Upsert an embedding record (append to journal + update in-memory index).
    pub async fn upsert(&mut self, record: EmbeddingRecord) -> Result<()> {
        self.ensure_loaded().await?;

        let entry = EmbeddingJournalEntry::Upsert {
            record: record.clone(),
        };
        self.append_entry(&entry).await?;
        self.journal_len += 1;
        self.index.insert(record.id.clone(), record);

        // Auto-compact when journal is significantly larger than live records
        if self.journal_len > self.index.len() + 100 {
            self.compact().await?;
        }

        Ok(())
    }

    /// Delete an embedding by ID (write tombstone + remove from index).
    pub async fn delete(&mut self, id: &str) -> Result<()> {
        self.ensure_loaded().await?;

        let entry = EmbeddingJournalEntry::Delete { id: id.to_string() };
        self.append_entry(&entry).await?;
        self.journal_len += 1;
        self.index.remove(id);

        Ok(())
    }

    /// Get a single embedding record by ID.
    pub async fn get(&mut self, id: &str) -> Result<Option<&EmbeddingRecord>> {
        self.ensure_loaded().await?;
        Ok(self.index.get(id))
    }

    /// Get all embedding records (for brute-force search).
    pub async fn get_all(&mut self) -> Result<&HashMap<String, EmbeddingRecord>> {
        self.ensure_loaded().await?;
        Ok(&self.index)
    }

    /// Compact the JSONL file by rewriting only live records.
    pub async fn compact(&mut self) -> Result<()> {
        self.ensure_loaded().await?;

        let tmp_path = self.file_path.with_extension("jsonl.tmp");
        let mut file = tokio::fs::File::create(&tmp_path).await.map_err(|e| {
            common::ToolError::ExecutionFailed(format!("Failed to create compact temp file: {}", e))
        })?;

        for record in self.index.values() {
            let entry = EmbeddingJournalEntry::Upsert {
                record: record.clone(),
            };
            let line = serde_json::to_string(&entry).map_err(|e| {
                common::ToolError::ExecutionFailed(format!("Failed to serialize record: {}", e))
            })?;
            file.write_all(line.as_bytes()).await.map_err(|e| {
                common::ToolError::ExecutionFailed(format!("Failed to write compact entry: {}", e))
            })?;
            file.write_all(b"\n").await.map_err(|e| {
                common::ToolError::ExecutionFailed(format!(
                    "Failed to write compact newline: {}",
                    e
                ))
            })?;
        }

        file.flush().await.map_err(|e| {
            common::ToolError::ExecutionFailed(format!("Failed to flush compact file: {}", e))
        })?;

        tokio::fs::rename(&tmp_path, &self.file_path)
            .await
            .map_err(|e| {
                common::ToolError::ExecutionFailed(format!("Failed to rename compact file: {}", e))
            })?;

        self.journal_len = self.index.len();
        Ok(())
    }

    /// Find todo IDs that don't have embeddings in the store.
    pub fn ids_missing_embeddings(&self, todo_ids: &[String]) -> Vec<String> {
        todo_ids
            .iter()
            .filter(|id| !self.index.contains_key(id.as_str()))
            .cloned()
            .collect()
    }

    /// Append a single journal entry to the JSONL file.
    async fn append_entry(&self, entry: &EmbeddingJournalEntry) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.file_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                common::ToolError::ExecutionFailed(format!(
                    "Failed to create embeddings directory: {}",
                    e
                ))
            })?;
        }

        let line = serde_json::to_string(entry).map_err(|e| {
            common::ToolError::ExecutionFailed(format!(
                "Failed to serialize embedding entry: {}",
                e
            ))
        })?;

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .await
            .map_err(|e| {
                common::ToolError::ExecutionFailed(format!("Failed to open embeddings file: {}", e))
            })?;

        file.write_all(line.as_bytes()).await.map_err(|e| {
            common::ToolError::ExecutionFailed(format!("Failed to write embedding entry: {}", e))
        })?;
        file.write_all(b"\n").await.map_err(|e| {
            common::ToolError::ExecutionFailed(format!("Failed to write embedding newline: {}", e))
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_store() -> (EmbeddingStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("embeddings.jsonl");
        let store = EmbeddingStore::new(file_path);
        (store, temp_dir)
    }

    fn create_test_record(id: &str) -> EmbeddingRecord {
        EmbeddingRecord {
            id: id.to_string(),
            embedding: vec![0.1; EMBEDDING_DIM],
            model: "test-model".to_string(),
            embedded_at: Utc::now(),
        }
    }

    // ─── JSONL Persistence Tests ──────────────────────────────────

    #[tokio::test]
    async fn test_jsonl_round_trip() {
        let (mut store, _dir) = create_test_store().await;
        let record = create_test_record("abc123");
        store.upsert(record.clone()).await.unwrap();

        // Create a new store pointing to the same file
        let mut store2 = EmbeddingStore::new(store.file_path().to_path_buf());
        store2.load().await.unwrap();

        let loaded = store2.get("abc123").await.unwrap().unwrap();
        assert_eq!(loaded.id, "abc123");
        assert_eq!(loaded.embedding.len(), EMBEDDING_DIM);
    }

    #[tokio::test]
    async fn test_upsert_overwrites_existing() {
        let (mut store, _dir) = create_test_store().await;

        let mut record1 = create_test_record("abc123");
        record1.embedding = vec![0.1; EMBEDDING_DIM];
        store.upsert(record1).await.unwrap();

        let mut record2 = create_test_record("abc123");
        record2.embedding = vec![0.9; EMBEDDING_DIM];
        store.upsert(record2).await.unwrap();

        let loaded = store.get("abc123").await.unwrap().unwrap();
        assert_eq!(loaded.embedding[0], 0.9);
    }

    // ─── Delete Tests ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_tombstone() {
        let (mut store, _dir) = create_test_store().await;
        let record = create_test_record("abc123");
        store.upsert(record).await.unwrap();
        store.delete("abc123").await.unwrap();

        // Reload
        let mut store2 = EmbeddingStore::new(store.file_path().to_path_buf());
        store2.load().await.unwrap();
        assert!(store2.get("abc123").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_id_is_noop() {
        let (mut store, _dir) = create_test_store().await;
        store.load().await.unwrap();
        let result = store.delete("nonexistent").await;
        assert!(result.is_ok());
    }

    // ─── Compaction Tests ─────────────────────────────────────────

    #[tokio::test]
    async fn test_compact_removes_stale_entries() {
        let (mut store, _dir) = create_test_store().await;

        // Write 100 upserts for the same ID (creates 100 journal entries)
        for i in 0..100 {
            let mut record = create_test_record("abc123");
            record.embedding = vec![i as f32 / 100.0; EMBEDDING_DIM];
            store.upsert(record).await.unwrap();
        }

        let size_before = std::fs::metadata(store.file_path()).unwrap().len();
        store.compact().await.unwrap();
        let size_after = std::fs::metadata(store.file_path()).unwrap().len();
        assert!(
            size_after < size_before,
            "Compaction should reduce file size"
        );

        // Verify data integrity
        let loaded = store.get("abc123").await.unwrap().unwrap();
        assert_eq!(loaded.embedding[0], 99.0 / 100.0);
    }

    #[tokio::test]
    async fn test_compact_preserves_all_live_records() {
        let (mut store, _dir) = create_test_store().await;
        for i in 0..50 {
            store
                .upsert(create_test_record(&format!("id-{}", i)))
                .await
                .unwrap();
        }
        store.compact().await.unwrap();

        let all = store.get_all().await.unwrap();
        assert_eq!(all.len(), 50);
    }

    // ─── Corrupted Data Recovery Tests ────────────────────────────

    #[tokio::test]
    async fn test_corrupted_line_recovery() {
        let (store, _dir) = create_test_store().await;
        let file_path = store.file_path().to_path_buf();

        let valid_record = serde_json::json!({
            "_op": "upsert",
            "record": {
                "id": "valid1",
                "embedding": vec![0.1f32; 384],
                "model": "test",
                "embedded_at": "2026-01-01T00:00:00Z"
            }
        });
        let valid_record2 = serde_json::json!({
            "_op": "upsert",
            "record": {
                "id": "valid2",
                "embedding": vec![0.2f32; 384],
                "model": "test",
                "embedded_at": "2026-01-01T00:00:00Z"
            }
        });
        let content = format!(
            "{}\nTHIS IS CORRUPTED\n{}\n",
            serde_json::to_string(&valid_record).unwrap(),
            serde_json::to_string(&valid_record2).unwrap(),
        );
        std::fs::write(&file_path, content).unwrap();

        let mut store2 = EmbeddingStore::new(file_path);
        store2.load().await.unwrap(); // Should not panic

        assert!(store2.get("valid1").await.unwrap().is_some());
        assert!(store2.get("valid2").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_empty_file_loads_ok() {
        let (mut store, _dir) = create_test_store().await;
        store.load().await.unwrap();
        let all = store.get_all().await.unwrap();
        assert!(all.is_empty());
    }

    #[tokio::test]
    async fn test_dimension_mismatch_skipped_on_load() {
        let (store, _dir) = create_test_store().await;
        let file_path = store.file_path().to_path_buf();

        let bad_record = serde_json::json!({
            "_op": "upsert",
            "record": {
                "id": "bad_dim",
                "embedding": vec![0.1f32; 128],
                "model": "wrong-model",
                "embedded_at": "2026-01-01T00:00:00Z"
            }
        });
        std::fs::write(
            &file_path,
            format!("{}\n", serde_json::to_string(&bad_record).unwrap()),
        )
        .unwrap();

        let mut store2 = EmbeddingStore::new(file_path);
        store2.load().await.unwrap();
        assert!(store2.get("bad_dim").await.unwrap().is_none());
    }

    // ─── Backfill Helper Tests ────────────────────────────────────

    #[tokio::test]
    async fn test_ids_missing_embeddings() {
        let (mut store, _dir) = create_test_store().await;
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
        let (store, _dir) = create_test_store().await;
        let all_ids = vec!["a".to_string(), "b".to_string()];
        let missing = store.ids_missing_embeddings(&all_ids);
        assert_eq!(missing.len(), 2);
    }
}
