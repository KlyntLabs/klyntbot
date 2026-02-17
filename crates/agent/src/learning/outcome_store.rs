//! JSONL-based outcome storage for the learning system.
//!
//! Follows the same append-only journal pattern as `EmbeddingStore`:
//! - Upserts and enrichment feedback are appended as journal entries
//! - On load, entries are replayed to reconstruct the in-memory index
//! - Compaction rewrites the file with only live records
//! - Corrupted lines are skipped with a warning (graceful degradation)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tracing::warn;

use common::Result;

use super::types::{EnrichmentFeedbackEntry, OutcomeRecord};

/// Journal entry for the JSONL outcome store.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "_op")]
enum OutcomeJournalEntry {
    #[serde(rename = "record")]
    Record { outcome: OutcomeRecord },
    #[serde(rename = "feedback")]
    Feedback { entry: EnrichmentFeedbackEntry },
}

/// In-memory outcome index backed by a JSONL file.
pub struct OutcomeStore {
    file_path: PathBuf,
    outcomes: Vec<OutcomeRecord>,
    feedback: Vec<EnrichmentFeedbackEntry>,
    loaded: bool,
    journal_len: usize,
}

impl OutcomeStore {
    /// Create a new store pointing to the given JSONL file.
    /// The file is NOT loaded until `load()` is called (lazy).
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            outcomes: Vec::new(),
            feedback: Vec::new(),
            loaded: false,
            journal_len: 0,
        }
    }

    /// Get the file path for this store.
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    /// Load (replay) the JSONL journal from disk into the in-memory index.
    pub async fn load(&mut self) -> Result<()> {
        self.outcomes.clear();
        self.feedback.clear();
        self.journal_len = 0;

        if !self.file_path.exists() {
            self.loaded = true;
            return Ok(());
        }

        let content = tokio::fs::read_to_string(&self.file_path)
            .await
            .map_err(|e| {
                common::ToolError::ExecutionFailed(format!(
                    "Failed to read outcomes file: {}",
                    e
                ))
            })?;

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            self.journal_len += 1;

            match serde_json::from_str::<OutcomeJournalEntry>(line) {
                Ok(OutcomeJournalEntry::Record { outcome }) => {
                    self.outcomes.push(outcome);
                }
                Ok(OutcomeJournalEntry::Feedback { entry }) => {
                    self.feedback.push(entry);
                }
                Err(e) => {
                    warn!(
                        "Skipping corrupted outcome line {} in {:?}: {}",
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

    /// Record a tool execution outcome.
    pub async fn record(&mut self, outcome: OutcomeRecord) -> Result<()> {
        self.ensure_loaded().await?;

        let entry = OutcomeJournalEntry::Record {
            outcome: outcome.clone(),
        };
        self.append_entry(&entry).await?;
        self.outcomes.push(outcome);

        // Auto-compact when journal is significantly larger than live records
        let live_count = self.outcomes.len() + self.feedback.len();
        if self.journal_len > live_count + 200 {
            self.compact().await?;
        }

        Ok(())
    }

    /// Record enrichment feedback.
    pub async fn record_feedback(&mut self, feedback: EnrichmentFeedbackEntry) -> Result<()> {
        self.ensure_loaded().await?;

        let entry = OutcomeJournalEntry::Feedback {
            entry: feedback.clone(),
        };
        self.append_entry(&entry).await?;
        self.feedback.push(feedback);

        Ok(())
    }

    /// Get all outcome records.
    pub async fn get_all_outcomes(&mut self) -> Result<&[OutcomeRecord]> {
        self.ensure_loaded().await?;
        Ok(&self.outcomes)
    }

    /// Get all enrichment feedback entries.
    pub async fn get_all_feedback(&mut self) -> Result<&[EnrichmentFeedbackEntry]> {
        self.ensure_loaded().await?;
        Ok(&self.feedback)
    }

    /// Get outcomes recorded after a given timestamp.
    pub async fn outcomes_since(
        &mut self,
        cutoff: DateTime<Utc>,
    ) -> Result<Vec<OutcomeRecord>> {
        self.ensure_loaded().await?;
        Ok(self
            .outcomes
            .iter()
            .filter(|o| o.created_at >= cutoff)
            .cloned()
            .collect())
    }

    /// Compact the JSONL file by rewriting only live records.
    pub async fn compact(&mut self) -> Result<()> {
        self.ensure_loaded().await?;

        let tmp_path = self.file_path.with_extension("jsonl.tmp");
        let mut file = tokio::fs::File::create(&tmp_path).await.map_err(|e| {
            common::ToolError::ExecutionFailed(format!(
                "Failed to create compact temp file: {}",
                e
            ))
        })?;

        for outcome in &self.outcomes {
            let entry = OutcomeJournalEntry::Record {
                outcome: outcome.clone(),
            };
            let line = serde_json::to_string(&entry).map_err(|e| {
                common::ToolError::ExecutionFailed(format!(
                    "Failed to serialize outcome: {}",
                    e
                ))
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

        for fb in &self.feedback {
            let entry = OutcomeJournalEntry::Feedback { entry: fb.clone() };
            let line = serde_json::to_string(&entry).map_err(|e| {
                common::ToolError::ExecutionFailed(format!(
                    "Failed to serialize feedback: {}",
                    e
                ))
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

        file.flush().await.map_err(|e| {
            common::ToolError::ExecutionFailed(format!(
                "Failed to flush compact file: {}",
                e
            ))
        })?;

        file.sync_data().await.map_err(|e| {
            common::ToolError::ExecutionFailed(format!(
                "Failed to sync compact file: {}",
                e
            ))
        })?;

        tokio::fs::rename(&tmp_path, &self.file_path)
            .await
            .map_err(|e| {
                common::ToolError::ExecutionFailed(format!(
                    "Failed to rename compact file: {}",
                    e
                ))
            })?;

        self.journal_len = self.outcomes.len() + self.feedback.len();
        Ok(())
    }

    /// Append a single journal entry to the JSONL file.
    async fn append_entry(&mut self, entry: &OutcomeJournalEntry) -> Result<()> {
        if let Some(parent) = self.file_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                common::ToolError::ExecutionFailed(format!(
                    "Failed to create outcomes directory: {}",
                    e
                ))
            })?;
        }

        let line = serde_json::to_string(entry).map_err(|e| {
            common::ToolError::ExecutionFailed(format!(
                "Failed to serialize outcome entry: {}",
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
                    "Failed to open outcomes file: {}",
                    e
                ))
            })?;

        file.write_all(line.as_bytes()).await.map_err(|e| {
            common::ToolError::ExecutionFailed(format!(
                "Failed to write outcome entry: {}",
                e
            ))
        })?;
        file.write_all(b"\n").await.map_err(|e| {
            common::ToolError::ExecutionFailed(format!(
                "Failed to write outcome newline: {}",
                e
            ))
        })?;

        self.journal_len += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::confidence::types::ConfidenceDimensions;
    use crate::learning::types::ExecutionMode;
    use tempfile::TempDir;

    async fn create_test_store() -> (OutcomeStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("outcomes.jsonl");
        let store = OutcomeStore::new(file_path);
        (store, temp_dir)
    }

    fn create_test_outcome(id: &str) -> OutcomeRecord {
        OutcomeRecord {
            id: id.to_string(),
            session_key: "test:hash".to_string(),
            tool_name: "todo".to_string(),
            success: true,
            error_category: None,
            duration_ms: 42,
            confidence_score: Some(0.85),
            confidence_dimensions: Some(ConfidenceDimensions {
                intent_clarity: 0.9,
                tool_fit: 0.8,
                info_sufficiency: 0.85,
            }),
            execution_mode: ExecutionMode::Chat,
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_jsonl_round_trip() {
        let (mut store, _dir) = create_test_store().await;
        let outcome = create_test_outcome("test-001");
        store.record(outcome).await.unwrap();

        let mut store2 = OutcomeStore::new(store.file_path().to_path_buf());
        store2.load().await.unwrap();

        let outcomes = store2.get_all_outcomes().await.unwrap();
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].id, "test-001");
    }

    #[tokio::test]
    async fn test_feedback_round_trip() {
        let (mut store, _dir) = create_test_store().await;
        let feedback = EnrichmentFeedbackEntry {
            task_id: "todo-123".to_string(),
            field: "priority".to_string(),
            suggested_value: "1".to_string(),
            actual_value: None,
            accepted: true,
            confidence: 0.85,
            timestamp: Utc::now(),
        };
        store.record_feedback(feedback).await.unwrap();

        let mut store2 = OutcomeStore::new(store.file_path().to_path_buf());
        store2.load().await.unwrap();

        let fb = store2.get_all_feedback().await.unwrap();
        assert_eq!(fb.len(), 1);
        assert_eq!(fb[0].task_id, "todo-123");
        assert!(fb[0].accepted);
    }

    #[tokio::test]
    async fn test_corrupted_line_recovery() {
        let (store, _dir) = create_test_store().await;
        let file_path = store.file_path().to_path_buf();

        let valid = serde_json::json!({
            "_op": "record",
            "outcome": {
                "id": "valid1",
                "session_key": "test:hash",
                "tool_name": "todo",
                "success": true,
                "error_category": null,
                "duration_ms": 42,
                "confidence_score": 0.85,
                "confidence_dimensions": null,
                "execution_mode": "chat",
                "created_at": "2026-01-01T00:00:00Z"
            }
        });
        let content = format!(
            "{}\nTHIS IS CORRUPTED\n",
            serde_json::to_string(&valid).unwrap()
        );
        std::fs::write(&file_path, content).unwrap();

        let mut store2 = OutcomeStore::new(file_path);
        store2.load().await.unwrap();

        let outcomes = store2.get_all_outcomes().await.unwrap();
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].id, "valid1");
    }

    #[tokio::test]
    async fn test_empty_file_loads_ok() {
        let (mut store, _dir) = create_test_store().await;
        store.load().await.unwrap();
        let outcomes = store.get_all_outcomes().await.unwrap();
        assert!(outcomes.is_empty());
    }

    #[tokio::test]
    async fn test_compact_preserves_all_records() {
        let (mut store, _dir) = create_test_store().await;

        // Write outcomes + some feedback
        for i in 0..20 {
            store
                .record(create_test_outcome(&format!("id-{}", i)))
                .await
                .unwrap();
        }
        store
            .record_feedback(EnrichmentFeedbackEntry {
                task_id: "todo-1".to_string(),
                field: "priority".to_string(),
                suggested_value: "1".to_string(),
                actual_value: None,
                accepted: true,
                confidence: 0.8,
                timestamp: Utc::now(),
            })
            .await
            .unwrap();

        store.compact().await.unwrap();

        // Reload and verify all records survived compaction
        let mut store2 = OutcomeStore::new(store.file_path().to_path_buf());
        store2.load().await.unwrap();

        let outcomes = store2.get_all_outcomes().await.unwrap();
        assert_eq!(outcomes.len(), 20);
        let feedback = store2.get_all_feedback().await.unwrap();
        assert_eq!(feedback.len(), 1);
    }

    #[tokio::test]
    async fn test_outcomes_since_filter() {
        let (mut store, _dir) = create_test_store().await;

        let cutoff = Utc::now();

        // Small delay to ensure timestamp ordering
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        store.record(create_test_outcome("after-cutoff")).await.unwrap();

        let filtered = store.outcomes_since(cutoff).await.unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "after-cutoff");
    }
}
