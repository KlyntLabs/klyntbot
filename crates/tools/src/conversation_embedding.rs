//! Conversation embedding storage and handler for semantic memory search.
//!
//! Follows the same append-only journal pattern as `EmbeddingStore`:
//! - Upserts and deletes are appended as journal entries
//! - On load, entries are replayed to reconstruct the in-memory index
//! - Compaction rewrites the file with only live records
//! - Corrupted lines are skipped with a warning (graceful degradation)
//!
//! The `ConversationEmbeddingHandler` trait provides dependency inversion for
//! the embedding pipeline (defined in tools crate L3, implemented in agent crate L5).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tokio::sync::{OnceCell as AsyncOnceCell, RwLock as TokioRwLock};
use tracing::warn;

use crate::embedding_engine::EMBEDDING_DIM;
use common::Result;

/// A single conversation embedding record persisted to the JSONL store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationEmbeddingRecord {
    pub id: String,              // Message UUID from SessionMessage.id
    pub session_key: String,     // "channel:chat_id"
    pub role: String,            // "user" | "assistant"
    pub content_preview: String, // First 100 chars
    pub embedding: Vec<f32>,     // 384 dimensions
    pub model: String,
    pub embedded_at: DateTime<Utc>,
}

/// Journal entry for append-only JSONL storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "_op")]
enum ConversationEmbeddingJournalEntry {
    #[serde(rename = "upsert")]
    Upsert { record: ConversationEmbeddingRecord },
    #[serde(rename = "delete")]
    Delete { id: String },
}

/// Filter for purging embeddings.
#[derive(Debug, Clone)]
pub enum PurgeFilter {
    /// Delete all embeddings for a specific session.
    BySessionKey(String),
    /// Delete all embeddings embedded before a specific date.
    Before(DateTime<Utc>),
    /// Delete all embeddings.
    All,
}

/// Status information about the conversation embedding store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationEmbeddingStatus {
    pub total_embeddings: usize,
    pub indexed_channels: Vec<String>,
    pub oldest_embedding: Option<DateTime<Utc>>,
    pub newest_embedding: Option<DateTime<Utc>>,
    pub is_available: bool,
}

/// In-memory conversation embedding index backed by a JSONL file or SQL (pgvector).
/// Uses interior mutability to allow concurrent reads via `&self` methods.
pub struct ConversationEmbeddingStore {
    file_path: PathBuf,
    index: TokioRwLock<HashMap<String, ConversationEmbeddingRecord>>,
    loaded: AsyncOnceCell<()>,
    journal_len: TokioRwLock<usize>,
    sql_repo: Option<storage::ConvEmbeddingRepo>,
}

impl ConversationEmbeddingStore {
    /// Create a new store pointing to the given JSONL file.
    /// The file is NOT loaded until first access (lazy via AsyncOnceCell).
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            index: TokioRwLock::new(HashMap::new()),
            loaded: AsyncOnceCell::new(),
            journal_len: TokioRwLock::new(0),
            sql_repo: None,
        }
    }

    /// Create a store backed by SQL (pgvector) via `ConvEmbeddingRepo`.
    pub fn from_repo(repo: storage::ConvEmbeddingRepo) -> Self {
        Self {
            file_path: PathBuf::from("/dev/null"),
            index: TokioRwLock::new(HashMap::new()),
            loaded: AsyncOnceCell::const_new_with(()),
            journal_len: TokioRwLock::new(0),
            sql_repo: Some(repo),
        }
    }

    /// Load (replay) the JSONL journal from disk into the in-memory index.
    /// Corrupted or dimension-mismatched lines are skipped with a warning.
    /// This method is private and called once via `ensure_loaded()`.
    async fn load(&self) -> Result<()> {
        let mut index = self.index.write().await;
        let mut journal_len = self.journal_len.write().await;

        index.clear();
        *journal_len = 0;

        if !self.file_path.exists() {
            return Ok(());
        }

        let content = tokio::fs::read_to_string(&self.file_path)
            .await
            .map_err(|e| {
                common::ToolError::ExecutionFailed(format!(
                    "Failed to read conversation embeddings file: {}",
                    e
                ))
            })?;

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            *journal_len += 1;

            match serde_json::from_str::<ConversationEmbeddingJournalEntry>(line) {
                Ok(ConversationEmbeddingJournalEntry::Upsert { record }) => {
                    // Validate dimension
                    if record.embedding.len() != EMBEDDING_DIM {
                        warn!(
                            "Skipping conversation embedding for {} at line {}: dimension {} != expected {}",
                            record.id,
                            line_num + 1,
                            record.embedding.len(),
                            EMBEDDING_DIM
                        );
                        continue;
                    }
                    index.insert(record.id.clone(), record);
                }
                Ok(ConversationEmbeddingJournalEntry::Delete { id }) => {
                    index.remove(&id);
                }
                Err(e) => {
                    warn!(
                        "Skipping corrupted conversation embedding line {} in {:?}: {}",
                        line_num + 1,
                        self.file_path,
                        e
                    );
                }
            }
        }

        Ok(())
    }

    /// Upsert an embedding record (append to journal + update in-memory index).
    /// In SQL mode, delegates to `ConvEmbeddingRepo::insert`.
    pub async fn upsert(&self, record: ConversationEmbeddingRecord) -> Result<()> {
        self.ensure_loaded().await?;

        if let Some(repo) = &self.sql_repo {
            let id = uuid::Uuid::parse_str(&record.id).unwrap_or_else(|_| uuid::Uuid::new_v4());
            let vector = pgvector::Vector::from(record.embedding.clone());
            // Delete existing first (upsert semantics)
            let _ = repo.delete(id).await;
            repo.insert(
                id,
                &record.session_key,
                &vector,
                &record.role,
                &record.content_preview,
            )
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;
            let mut index = self.index.write().await;
            index.insert(record.id.clone(), record);
            return Ok(());
        }

        let entry = ConversationEmbeddingJournalEntry::Upsert {
            record: record.clone(),
        };
        self.append_entry(&entry).await?;

        {
            let mut journal_len = self.journal_len.write().await;
            *journal_len += 1;
        }

        {
            let mut index = self.index.write().await;
            index.insert(record.id.clone(), record);
        }

        // Auto-compact when journal is significantly larger than live records
        let journal_len = *self.journal_len.read().await;
        let index_len = self.index.read().await.len();
        if journal_len > index_len + 100 {
            self.compact().await?;
        }

        Ok(())
    }

    /// Get a single embedding record by message ID.
    pub async fn get(&self, id: &str) -> Result<Option<ConversationEmbeddingRecord>> {
        self.ensure_loaded().await?;
        let index = self.index.read().await;
        Ok(index.get(id).cloned())
    }

    /// Get all embedding records matching a session key.
    pub async fn get_by_session_key(
        &self,
        session_key: &str,
    ) -> Result<Vec<ConversationEmbeddingRecord>> {
        self.ensure_loaded().await?;
        let index = self.index.read().await;
        Ok(index
            .values()
            .filter(|r| r.session_key == session_key)
            .cloned()
            .collect())
    }

    /// Get all embedding records (for brute-force search).
    pub async fn get_all(&self) -> Result<HashMap<String, ConversationEmbeddingRecord>> {
        self.ensure_loaded().await?;
        let index = self.index.read().await;
        Ok(index.clone())
    }

    /// Get status information about the store.
    pub async fn status(&self) -> Result<ConversationEmbeddingStatus> {
        self.ensure_loaded().await?;

        let index = self.index.read().await;
        let mut indexed_channels: Vec<String> = index
            .values()
            .map(|r| r.session_key.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        indexed_channels.sort();

        let oldest_embedding = index.values().map(|r| r.embedded_at).min();

        let newest_embedding = index.values().map(|r| r.embedded_at).max();

        Ok(ConversationEmbeddingStatus {
            total_embeddings: index.len(),
            indexed_channels,
            oldest_embedding,
            newest_embedding,
            is_available: true,
        })
    }

    /// Purge embeddings matching the filter.
    /// In SQL mode, fetches matching IDs from the in-memory index and deletes via repo.
    pub async fn purge(&self, filter: PurgeFilter) -> Result<usize> {
        self.ensure_loaded().await?;

        if let Some(repo) = &self.sql_repo {
            let index = self.index.read().await;
            let ids_to_delete: Vec<String> = match &filter {
                PurgeFilter::BySessionKey(session_key) => index
                    .values()
                    .filter(|r| r.session_key == *session_key)
                    .map(|r| r.id.clone())
                    .collect(),
                PurgeFilter::Before(cutoff) => index
                    .values()
                    .filter(|r| r.embedded_at < *cutoff)
                    .map(|r| r.id.clone())
                    .collect(),
                PurgeFilter::All => index.keys().cloned().collect(),
            };
            drop(index);

            let count = ids_to_delete.len();
            for id in &ids_to_delete {
                if let Ok(uuid) = uuid::Uuid::parse_str(id) {
                    let _ = repo.delete(uuid).await;
                }
            }
            let mut index = self.index.write().await;
            for id in &ids_to_delete {
                index.remove(id);
            }
            return Ok(count);
        }

        let ids_to_delete: Vec<String> = {
            let index = self.index.read().await;
            match filter {
                PurgeFilter::BySessionKey(session_key) => index
                    .values()
                    .filter(|r| r.session_key == session_key)
                    .map(|r| r.id.clone())
                    .collect(),
                PurgeFilter::Before(cutoff) => index
                    .values()
                    .filter(|r| r.embedded_at < cutoff)
                    .map(|r| r.id.clone())
                    .collect(),
                PurgeFilter::All => index.keys().cloned().collect(),
            }
        };

        let count = ids_to_delete.len();

        for id in ids_to_delete {
            let entry = ConversationEmbeddingJournalEntry::Delete { id: id.clone() };
            self.append_entry(&entry).await?;

            {
                let mut journal_len = self.journal_len.write().await;
                *journal_len += 1;
            }

            {
                let mut index = self.index.write().await;
                index.remove(&id);
            }
        }

        Ok(count)
    }

    /// Compact the JSONL file by rewriting only live records.
    /// In SQL mode, this is a no-op (no journal to compact).
    pub async fn compact(&self) -> Result<()> {
        if self.sql_repo.is_some() {
            return Ok(());
        }
        self.ensure_loaded().await?;

        let tmp_path = self.file_path.with_extension("jsonl.tmp");
        let mut file = tokio::fs::File::create(&tmp_path).await.map_err(|e| {
            common::ToolError::ExecutionFailed(format!("Failed to create compact temp file: {}", e))
        })?;

        {
            let index = self.index.read().await;
            for record in index.values() {
                let entry = ConversationEmbeddingJournalEntry::Upsert {
                    record: record.clone(),
                };
                let line = serde_json::to_string(&entry).map_err(|e| {
                    common::ToolError::ExecutionFailed(format!("Failed to serialize record: {}", e))
                })?;
                file.write_all(line.as_bytes()).await.map_err(|e| {
                    common::ToolError::ExecutionFailed(format!(
                        "Failed to write compact entry: {}",
                        e
                    ))
                })?;
                file.write_all(b"\n").await.map_err(|e| {
                    common::ToolError::ExecutionFailed(format!(
                        "Failed to write compact newline: {}",
                        e
                    ))
                })?;
            }
        }

        file.flush().await.map_err(|e| {
            common::ToolError::ExecutionFailed(format!("Failed to flush compact file: {}", e))
        })?;

        tokio::fs::rename(&tmp_path, &self.file_path)
            .await
            .map_err(|e| {
                common::ToolError::ExecutionFailed(format!("Failed to rename compact file: {}", e))
            })?;

        {
            let index_len = self.index.read().await.len();
            let mut journal_len = self.journal_len.write().await;
            *journal_len = index_len;
        }

        Ok(())
    }

    /// Append a single journal entry to the JSONL file.
    async fn append_entry(&self, entry: &ConversationEmbeddingJournalEntry) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.file_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                common::ToolError::ExecutionFailed(format!(
                    "Failed to create conversation embeddings directory: {}",
                    e
                ))
            })?;
        }

        let line = serde_json::to_string(entry).map_err(|e| {
            common::ToolError::ExecutionFailed(format!(
                "Failed to serialize conversation embedding entry: {}",
                e
            ))
        })?;

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .await
            .map_err(|e| {
                common::ToolError::ExecutionFailed(format!(
                    "Failed to open conversation embeddings file: {}",
                    e
                ))
            })?;

        file.write_all(line.as_bytes()).await.map_err(|e| {
            common::ToolError::ExecutionFailed(format!(
                "Failed to write conversation embedding entry: {}",
                e
            ))
        })?;
        file.write_all(b"\n").await.map_err(|e| {
            common::ToolError::ExecutionFailed(format!(
                "Failed to write conversation embedding newline: {}",
                e
            ))
        })?;

        Ok(())
    }

    /// Ensure the store is loaded before any read/write operation.
    /// Uses AsyncOnceCell for one-time async initialization.
    async fn ensure_loaded(&self) -> Result<()> {
        self.loaded.get_or_try_init(|| self.load()).await?;
        Ok(())
    }
}

/// ConversationEmbeddingHandler trait for dependency inversion.
/// Implemented by ConversationEmbeddingHandlerImpl in agent crate (Layer 5).
/// Defined here in tools crate (Layer 3) to break circular dependency.
#[async_trait]
pub trait ConversationEmbeddingHandler: Send + Sync {
    /// Embed a message and store it.
    async fn embed_message(
        &self,
        session_key: &str,
        role: &str,
        content: &str,
        message_id: &str,
    ) -> Result<()>;

    /// Search for similar conversations using cosine similarity.
    async fn search(
        &self,
        query: &str,
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<(ConversationEmbeddingRecord, f64)>>;

    /// Purge embeddings matching the filter.
    async fn purge(&self, filter: PurgeFilter) -> Result<usize>;

    /// Get status information about the embedding store.
    async fn status(&self) -> Result<ConversationEmbeddingStatus>;

    /// Check if the handler is available (model loaded, etc.).
    fn is_available(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ──────────────────────────────────────────────────────────────────────
    // Section 3.1: ConversationEmbeddingStore Unit Tests
    // ──────────────────────────────────────────────────────────────────────

    async fn create_test_store() -> (ConversationEmbeddingStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("conversation_embeddings.jsonl");
        let store = ConversationEmbeddingStore::new(file_path);
        (store, temp_dir)
    }

    fn create_test_record(id: &str, session_key: &str, role: &str) -> ConversationEmbeddingRecord {
        ConversationEmbeddingRecord {
            id: id.to_string(),
            session_key: session_key.to_string(),
            role: role.to_string(),
            content_preview: "Test message content".to_string(),
            embedding: vec![0.1; EMBEDDING_DIM],
            model: "test-model".to_string(),
            embedded_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_upsert_new_record() {
        let (store, _dir) = create_test_store().await;
        let record = create_test_record("msg-1", "telegram:12345", "user");

        store.upsert(record.clone()).await.unwrap();

        let loaded = store.get("msg-1").await.unwrap().unwrap();
        assert_eq!(loaded.id, "msg-1");
        assert_eq!(loaded.session_key, "telegram:12345");
        assert_eq!(loaded.role, "user");
        assert_eq!(loaded.embedding.len(), EMBEDDING_DIM);
    }

    #[tokio::test]
    async fn test_load_from_empty_file() {
        let (store, _dir) = create_test_store().await;

        store.load().await.unwrap();

        let all = store.get_all().await.unwrap();
        assert!(all.is_empty(), "Empty file should result in empty store");
    }

    #[tokio::test]
    async fn test_load_skips_corrupted_lines() {
        let (store, _dir) = create_test_store().await;
        let file_path = store.file_path.to_path_buf();

        // Write valid record, corrupted line, then another valid record
        let valid_record1 = serde_json::json!({
            "_op": "upsert",
            "record": {
                "id": "valid1",
                "session_key": "telegram:11111",
                "role": "user",
                "content_preview": "Valid message",
                "embedding": vec![0.1f32; 384],
                "model": "test",
                "embedded_at": "2026-01-01T00:00:00Z"
            }
        });
        let valid_record2 = serde_json::json!({
            "_op": "upsert",
            "record": {
                "id": "valid2",
                "session_key": "telegram:22222",
                "role": "assistant",
                "content_preview": "Another valid message",
                "embedding": vec![0.2f32; 384],
                "model": "test",
                "embedded_at": "2026-01-02T00:00:00Z"
            }
        });
        let content = format!(
            "{}\nTHIS IS CORRUPTED JSON\n{}\n",
            serde_json::to_string(&valid_record1).unwrap(),
            serde_json::to_string(&valid_record2).unwrap(),
        );
        std::fs::write(&file_path, content).unwrap();

        let store2 = ConversationEmbeddingStore::new(file_path);
        store2.load().await.unwrap(); // Should not panic

        assert!(
            store2.get("valid1").await.unwrap().is_some(),
            "Should load valid1"
        );
        assert!(
            store2.get("valid2").await.unwrap().is_some(),
            "Should load valid2"
        );
    }

    #[tokio::test]
    async fn test_search_by_session_key() {
        let (store, _dir) = create_test_store().await;

        store
            .upsert(create_test_record("msg-1", "telegram:12345", "user"))
            .await
            .unwrap();
        store
            .upsert(create_test_record("msg-2", "telegram:12345", "assistant"))
            .await
            .unwrap();
        store
            .upsert(create_test_record("msg-3", "discord:67890", "user"))
            .await
            .unwrap();

        let telegram_records = store.get_by_session_key("telegram:12345").await.unwrap();
        assert_eq!(telegram_records.len(), 2, "Should find 2 telegram messages");

        let discord_records = store.get_by_session_key("discord:67890").await.unwrap();
        assert_eq!(discord_records.len(), 1, "Should find 1 discord message");
    }

    #[tokio::test]
    async fn test_purge_by_session_key() {
        let (store, _dir) = create_test_store().await;

        store
            .upsert(create_test_record("msg-1", "telegram:12345", "user"))
            .await
            .unwrap();
        store
            .upsert(create_test_record("msg-2", "telegram:12345", "assistant"))
            .await
            .unwrap();
        store
            .upsert(create_test_record("msg-3", "discord:67890", "user"))
            .await
            .unwrap();

        let deleted = store
            .purge(PurgeFilter::BySessionKey("telegram:12345".to_string()))
            .await
            .unwrap();
        assert_eq!(deleted, 2, "Should delete 2 telegram messages");

        let remaining = store.get_all().await.unwrap();
        assert_eq!(remaining.len(), 1, "Should have 1 message remaining");
        assert!(
            remaining.contains_key("msg-3"),
            "Discord message should remain"
        );
    }

    #[tokio::test]
    async fn test_purge_by_date() {
        let (store, _dir) = create_test_store().await;

        let old_date = Utc::now() - chrono::Duration::days(10);
        let cutoff_date = Utc::now() - chrono::Duration::days(5);
        let recent_date = Utc::now();

        let mut old_record = create_test_record("msg-old", "telegram:12345", "user");
        old_record.embedded_at = old_date;
        store.upsert(old_record).await.unwrap();

        let mut recent_record = create_test_record("msg-recent", "telegram:12345", "user");
        recent_record.embedded_at = recent_date;
        store.upsert(recent_record).await.unwrap();

        let deleted = store.purge(PurgeFilter::Before(cutoff_date)).await.unwrap();
        assert_eq!(deleted, 1, "Should delete 1 old message");

        let remaining = store.get_all().await.unwrap();
        assert_eq!(remaining.len(), 1, "Should have 1 recent message remaining");
        assert!(
            remaining.contains_key("msg-recent"),
            "Recent message should remain"
        );
    }

    #[tokio::test]
    async fn test_compaction_removes_stale() {
        let (store, _dir) = create_test_store().await;

        // Write 100 upserts for the same ID (creates 100 journal entries)
        for i in 0..100 {
            let mut record = create_test_record("msg-1", "telegram:12345", "user");
            record.embedding = vec![i as f32 / 100.0; EMBEDDING_DIM];
            store.upsert(record).await.unwrap();
        }

        let size_before = std::fs::metadata(&store.file_path).unwrap().len();
        store.compact().await.unwrap();
        let size_after = std::fs::metadata(&store.file_path).unwrap().len();

        assert!(
            size_after < size_before,
            "Compaction should reduce file size"
        );

        // Verify data integrity
        let loaded = store.get("msg-1").await.unwrap().unwrap();
        assert_eq!(
            loaded.embedding[0],
            99.0 / 100.0,
            "Should have latest embedding"
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // Section 3.2: ConversationEmbeddingHandler Unit Tests
    // (These will be implemented as integration tests in the mock handler)
    // ──────────────────────────────────────────────────────────────────────

    // Note: Handler tests are deferred to mock_conversation_embedding_handler.rs
    // because they require the full handler implementation, which depends on
    // EmbeddingEngine (agent crate L5). We test the trait contract there.
}
