//! Strategy learning store — records execution strategy effectiveness.
//!
//! Appends `StrategyRecord` entries to a JSONL file and provides
//! strategy accuracy queries for adaptive orchestration.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tracing::warn;

use common::Result;

/// Records the outcome of a single strategy execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyRecord {
    pub timestamp: DateTime<Utc>,
    pub request_id: String,
    /// Predicted strategy name (e.g., "DirectResponse", "ToolAssisted").
    pub predicted_strategy: String,
    /// Actually used strategy (may differ if escalation occurred).
    pub actual_strategy: String,
    /// Number of times the request escalated to a different strategy.
    pub escalation_count: u32,
    /// Tool iterations actually used.
    pub iterations_used: u32,
    /// Maximum iterations allowed by the strategy.
    pub max_iterations: u32,
    pub success: bool,
    /// Optional user satisfaction score (filled later via feedback).
    pub user_satisfaction: Option<f32>,
    /// End-to-end response time in milliseconds.
    pub response_time_ms: u64,
}

/// JSONL-backed (or SQL-backed) store for strategy execution records.
pub struct StrategyLearningStore {
    file_path: PathBuf,
    records: Vec<StrategyRecord>,
    loaded: bool,
    sql_repo: Option<storage::StrategyRepo>,
}

impl StrategyLearningStore {
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            records: Vec::new(),
            loaded: false,
            sql_repo: None,
        }
    }

    /// Create a store backed by a SQL repository.
    pub fn from_repo(repo: storage::StrategyRepo) -> Self {
        Self {
            file_path: PathBuf::new(),
            records: Vec::new(),
            loaded: true, // no JSONL to load
            sql_repo: Some(repo),
        }
    }

    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    /// Load existing records from the JSONL file (no-op for SQL backend).
    pub async fn load(&mut self) -> Result<()> {
        if self.sql_repo.is_some() {
            return Ok(());
        }

        self.records.clear();
        self.loaded = true;

        let content = match tokio::fs::read_to_string(&self.file_path).await {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            match serde_json::from_str::<StrategyRecord>(trimmed) {
                Ok(record) => self.records.push(record),
                Err(e) => warn!("Skipping corrupt strategy record: {}", e),
            }
        }

        Ok(())
    }

    /// Append a strategy record to the JSONL file (or SQL) and in-memory index.
    pub async fn record_strategy(&mut self, record: StrategyRecord) -> Result<()> {
        // SQL path
        if let Some(repo) = &self.sql_repo {
            let row = storage::StrategyRecordRow {
                id: uuid::Uuid::new_v4(),
                timestamp: record.timestamp,
                request_id: record.request_id.clone(),
                predicted_strategy: record.predicted_strategy.clone(),
                actual_strategy: record.actual_strategy.clone(),
                escalation_count: record.escalation_count as i32,
                iterations_used: record.iterations_used as i32,
                max_iterations: record.max_iterations as i32,
                success: record.success,
                user_satisfaction: record.user_satisfaction,
                response_time_ms: record.response_time_ms as i64,
            };
            repo.create(&row)
                .await
                .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;
            self.records.push(record);
            return Ok(());
        }

        // JSONL path (original)
        // Ensure parent directory exists
        if let Some(parent) = self.file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut line = serde_json::to_string(&record)?;
        line.push('\n');

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .await?;
        file.write_all(line.as_bytes()).await?;
        file.flush().await?;

        self.records.push(record);
        Ok(())
    }

    /// Get the accuracy of a strategy over the last N days.
    ///
    /// Accuracy = fraction of records where `predicted_strategy == actual_strategy`
    /// (i.e., no escalation was needed).
    ///
    /// Returns `None` if no records exist for that strategy in the time window.
    ///
    /// For the SQL backend, this delegates to an async call internally using
    /// `tokio::task::block_in_place`. Use `get_strategy_accuracy_async` for a
    /// fully async version.
    pub fn get_strategy_accuracy(&self, strategy: &str, days: u32) -> Option<f32> {
        // SQL path — delegate synchronously (callers are typically in async context)
        if let Some(repo) = &self.sql_repo {
            let cutoff = Utc::now() - Duration::days(days as i64);
            let repo = repo.clone();
            let strategy = strategy.to_string();
            // Use block_in_place to bridge sync → async
            return tokio::task::block_in_place(move || {
                tokio::runtime::Handle::current().block_on(async {
                    repo.get_accuracy(&strategy, cutoff).await.ok().flatten()
                })
            });
        }

        // JSONL path (original)
        let cutoff = Utc::now() - Duration::days(days as i64);

        let matching: Vec<&StrategyRecord> = self
            .records
            .iter()
            .filter(|r| r.predicted_strategy == strategy && r.timestamp > cutoff)
            .collect();

        if matching.is_empty() {
            return None;
        }

        let correct = matching
            .iter()
            .filter(|r| r.predicted_strategy == r.actual_strategy)
            .count();

        Some(correct as f32 / matching.len() as f32)
    }

    /// Get all in-memory records (for analysis).
    ///
    /// Note: When using the SQL backend, only records created during this
    /// session are available in-memory. Historical records live in the database.
    pub fn records(&self) -> &[StrategyRecord] {
        &self.records
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_record(predicted: &str, actual: &str, success: bool) -> StrategyRecord {
        StrategyRecord {
            timestamp: Utc::now(),
            request_id: uuid::Uuid::new_v4().to_string(),
            predicted_strategy: predicted.to_string(),
            actual_strategy: actual.to_string(),
            escalation_count: if predicted == actual { 0 } else { 1 },
            iterations_used: 3,
            max_iterations: 10,
            success,
            user_satisfaction: None,
            response_time_ms: 150,
        }
    }

    #[tokio::test]
    async fn test_record_strategy_appends_to_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("strategy_learning.jsonl");
        let mut store = StrategyLearningStore::new(path.clone());

        let record = make_record("ToolAssisted", "ToolAssisted", true);
        store.record_strategy(record).await.unwrap();

        // Verify file was written
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(content.contains("ToolAssisted"));
        assert_eq!(content.lines().count(), 1);

        // Write a second record
        let record2 = make_record("DirectResponse", "DirectResponse", true);
        store.record_strategy(record2).await.unwrap();

        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(content.lines().count(), 2);
        assert_eq!(store.records().len(), 2);
    }

    #[tokio::test]
    async fn test_load_replays_records() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("strategy_learning.jsonl");

        // Write some records
        {
            let mut store = StrategyLearningStore::new(path.clone());
            store
                .record_strategy(make_record("ToolAssisted", "ToolAssisted", true))
                .await
                .unwrap();
            store
                .record_strategy(make_record("DirectResponse", "ToolAssisted", false))
                .await
                .unwrap();
        }

        // Load in a fresh store
        let mut store2 = StrategyLearningStore::new(path);
        store2.load().await.unwrap();
        assert_eq!(store2.records().len(), 2);
    }

    #[test]
    fn test_get_accuracy_returns_fraction() {
        let mut store = StrategyLearningStore::new(PathBuf::from("/tmp/unused"));
        // Manually inject records
        store
            .records
            .push(make_record("ToolAssisted", "ToolAssisted", true));
        store
            .records
            .push(make_record("ToolAssisted", "ToolAssisted", true));
        store
            .records
            .push(make_record("ToolAssisted", "AutonomousTask", false)); // escalation

        let accuracy = store.get_strategy_accuracy("ToolAssisted", 7).unwrap();
        assert!((accuracy - 2.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn test_empty_store_returns_none() {
        let store = StrategyLearningStore::new(PathBuf::from("/tmp/unused"));
        assert!(store.get_strategy_accuracy("ToolAssisted", 7).is_none());
    }

    #[test]
    fn test_accuracy_filters_by_strategy_name() {
        let mut store = StrategyLearningStore::new(PathBuf::from("/tmp/unused"));
        store
            .records
            .push(make_record("ToolAssisted", "ToolAssisted", true));
        store
            .records
            .push(make_record("DirectResponse", "DirectResponse", true));

        let ta_accuracy = store.get_strategy_accuracy("ToolAssisted", 7).unwrap();
        assert!((ta_accuracy - 1.0).abs() < f32::EPSILON);

        let dr_accuracy = store.get_strategy_accuracy("DirectResponse", 7).unwrap();
        assert!((dr_accuracy - 1.0).abs() < f32::EPSILON);

        assert!(store.get_strategy_accuracy("AutonomousTask", 7).is_none());
    }

    #[test]
    fn test_strategy_record_serde_round_trip() {
        let record = make_record("ToolAssisted", "AutonomousTask", false);
        let json = serde_json::to_string(&record).unwrap();
        let loaded: StrategyRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.predicted_strategy, "ToolAssisted");
        assert_eq!(loaded.actual_strategy, "AutonomousTask");
        assert!(!loaded.success);
    }
}
