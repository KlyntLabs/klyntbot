//! PlanStore - Append-only JSONL persistence for planning engine
//!
//! Mirrors the GoalStore pattern: append-only journal, periodic compaction,
//! lazy loading, and O(1) writes.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::types::{Plan, PlanStatus};
use common::Result;

/// Compaction threshold: compact when journal has this many more entries than live plans
const COMPACTION_THRESHOLD: usize = 100;

/// A single entry in the append-only journal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "_op")]
enum JournalEntry {
    #[serde(rename = "upsert")]
    Upsert { plan: Plan },
    #[serde(rename = "delete")]
    Delete { id: Uuid },
}

/// Append-only JSONL-backed plan storage with lazy loading and automatic compaction.
pub struct PlanStore {
    file_path: PathBuf,
    /// In-memory index: id -> Plan (authoritative state after load)
    index: HashMap<Uuid, Plan>,
    /// Ordered list of live plan IDs (preserves insertion order)
    order: Vec<Uuid>,
    loaded: bool,
    /// Number of journal entries on disk (including stale/overwritten ones)
    pub journal_len: usize,
}

impl PlanStore {
    /// Create a new PlanStore (does not load from disk yet).
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            index: HashMap::new(),
            order: Vec::new(),
            loaded: false,
            journal_len: 0,
        }
    }

    /// Ensure the store is loaded before any read operation.
    async fn ensure_loaded(&mut self) -> Result<()> {
        if !self.loaded {
            self.load().await?;
        }
        Ok(())
    }

    /// Load plans from JSONL file, replaying the journal to build the index.
    async fn load(&mut self) -> Result<()> {
        if !self.file_path.exists() {
            self.index = HashMap::new();
            self.order = Vec::new();
            self.loaded = true;
            self.journal_len = 0;
            return Ok(());
        }

        let content = fs::read_to_string(&self.file_path).await?;
        let mut index = HashMap::new();
        let mut order = Vec::new();
        let mut journal_len = 0;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            journal_len += 1;

            if let Ok(entry) = serde_json::from_str::<JournalEntry>(line) {
                match entry {
                    JournalEntry::Upsert { plan } => {
                        if !index.contains_key(&plan.id) {
                            order.push(plan.id);
                        }
                        index.insert(plan.id, plan);
                    }
                    JournalEntry::Delete { id } => {
                        if index.remove(&id).is_some() {
                            order.retain(|oid| oid != &id);
                        }
                    }
                }
            }
        }

        self.index = index;
        self.order = order;
        self.journal_len = journal_len;
        self.loaded = true;
        Ok(())
    }

    /// Append a single journal entry to the file (O(1) write).
    async fn append_entry(&mut self, entry: &JournalEntry) -> Result<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .await?;

        let mut line = serde_json::to_string(entry)?;
        line.push('\n');
        file.write_all(line.as_bytes()).await?;
        file.flush().await?;

        self.journal_len += 1;
        Ok(())
    }

    /// Compact the journal file: rewrite with only live entries.
    async fn compact(&mut self) -> Result<()> {
        // Create backup before compaction
        let backup_path = self.file_path.with_extension("jsonl.bak");
        if self.file_path.exists() {
            fs::copy(&self.file_path, &backup_path).await?;
        }

        let mut content = String::with_capacity(self.index.len() * 256);
        for id in &self.order {
            if let Some(plan) = self.index.get(id) {
                let entry = JournalEntry::Upsert { plan: plan.clone() };
                content.push_str(&serde_json::to_string(&entry)?);
                content.push('\n');
            }
        }

        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(&self.file_path, content).await?;
        self.journal_len = self.index.len();
        Ok(())
    }

    /// Check if compaction is needed and run it.
    async fn maybe_compact(&mut self) -> Result<()> {
        let stale = self.journal_len.saturating_sub(self.index.len());
        if stale >= COMPACTION_THRESHOLD {
            self.compact().await?;
        }
        Ok(())
    }

    /// Upsert a plan (insert or update).
    pub async fn upsert(&mut self, mut plan: Plan) -> Result<Plan> {
        self.ensure_loaded().await?;

        plan.updated_at = Utc::now();

        let entry = JournalEntry::Upsert { plan: plan.clone() };
        self.append_entry(&entry).await?;

        if !self.index.contains_key(&plan.id) {
            self.order.push(plan.id);
        }
        self.index.insert(plan.id, plan.clone());

        self.maybe_compact().await?;
        Ok(plan)
    }

    /// Get a plan by ID (O(1) lookup).
    pub async fn get(&mut self, id: &Uuid) -> Result<Option<Plan>> {
        self.ensure_loaded().await?;
        Ok(self.index.get(id).cloned())
    }

    /// Get the most recent active plan for a session (by updated_at timestamp).
    /// Returns Draft or Approved plans only, sorted by most recent first.
    pub async fn get_active_plan(&mut self, session_key: &str) -> Result<Option<Plan>> {
        self.ensure_loaded().await?;

        let mut active_plans: Vec<&Plan> = self
            .index
            .values()
            .filter(|p| {
                p.session_key == session_key
                    && (p.status == PlanStatus::Draft || p.status == PlanStatus::Approved)
            })
            .collect();

        // Sort by updated_at descending (most recent first)
        active_plans.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(active_plans.first().map(|p| (*p).clone()))
    }

    /// Delete a plan by ID.
    pub async fn delete(&mut self, id: &Uuid) -> Result<bool> {
        self.ensure_loaded().await?;

        if self.index.remove(id).is_some() {
            self.order.retain(|oid| oid != id);

            let entry = JournalEntry::Delete { id: *id };
            self.append_entry(&entry).await?;
            self.maybe_compact().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PlanStatus;
    use tempfile::tempdir;

    /// Helper function to create a minimal test Plan
    fn test_plan(title: &str, session_key: &str) -> Plan {
        let now = Utc::now();
        Plan {
            id: Uuid::new_v4(),
            session_key: session_key.to_string(),
            goal_id: None,
            title: title.to_string(),
            description: format!("Test plan: {}", title),
            status: PlanStatus::Draft,
            steps: vec![],
            current_step_index: 0,
            iteration_limit: 50,
            backtrack_history: vec![],
            created_at: now,
            updated_at: now,
            completed_at: None,
        }
    }

    #[tokio::test]
    async fn test_store_upsert_and_get() {
        // TODO: Test 6
        // Given: an empty PlanStore
        // When: a Plan is upserted and retrieved by ID
        // Then: the retrieved Plan matches the original
        // Maps to: US-2 (AC-2.2)

        let dir = tempdir().unwrap();
        let path = dir.path().join("plans.jsonl");
        let mut store = PlanStore::new(path);

        let plan = test_plan("Test Plan", "session-1");
        let id = plan.id;

        store.upsert(plan).await.unwrap();
        let retrieved = store.get(&id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title, "Test Plan");
    }

    #[tokio::test]
    async fn test_store_get_by_session() {
        // TODO: Test 7
        // Given: multiple Plans with different session_keys (session-A has 2 Draft plans)
        // When: get_active_plan("session-A") is called
        // Then: the most recent Draft/Approved plan by `updated_at` timestamp is returned
        // And: Plans from other sessions are not visible
        // Maps to: US-2 (AC-2.1)

        let dir = tempdir().unwrap();
        let path = dir.path().join("plans.jsonl");
        let mut store = PlanStore::new(path);

        let plan_a1 = test_plan("Plan A1", "session-A");
        let mut plan_a2 = test_plan("Plan A2", "session-A");
        let plan_b = test_plan("Plan B", "session-B");

        // Insert plans with small delay to ensure different timestamps
        store.upsert(plan_a1.clone()).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        plan_a2.updated_at = Utc::now();
        store.upsert(plan_a2.clone()).await.unwrap();
        store.upsert(plan_b).await.unwrap();

        let retrieved = store.get_active_plan("session-A").await.unwrap();
        assert!(retrieved.is_some());
        // Should get the most recent one (plan_a2)
        assert_eq!(retrieved.unwrap().title, "Plan A2");

        let retrieved_b = store.get_active_plan("session-B").await.unwrap();
        assert!(retrieved_b.is_some());
        assert_eq!(retrieved_b.unwrap().title, "Plan B");
    }

    #[tokio::test]
    async fn test_store_persistence_across_instances() {
        // TODO: Test 8
        // Given: a Plan saved to plans.jsonl
        // When: the store is dropped and a new instance loads the file
        // Then: the Plan is retrieved correctly from disk
        // Maps to: US-2 (data persistence)

        let dir = tempdir().unwrap();
        let path = dir.path().join("plans.jsonl");

        let plan = test_plan("Persistent Plan", "session-1");
        let id = plan.id;

        {
            let mut store = PlanStore::new(path.clone());
            store.upsert(plan).await.unwrap();
        } // Store dropped here

        {
            let mut store = PlanStore::new(path);
            let retrieved = store.get(&id).await.unwrap();
            assert!(retrieved.is_some());
            assert_eq!(retrieved.unwrap().title, "Persistent Plan");
        }
    }

    #[tokio::test]
    async fn test_store_compaction_threshold() {
        // TODO: Test 9
        // Given: 150 journal entries (upserts and deletes)
        // When: the COMPACTION_THRESHOLD (100) is exceeded
        // Then:
        //   - journal is compacted automatically
        //   - only live Plans remain
        //   - journal_len is reduced
        // Maps to: Infrastructure (JSONL performance)

        let dir = tempdir().unwrap();
        let path = dir.path().join("plans.jsonl");
        let mut store = PlanStore::new(path);

        for i in 0..150 {
            let plan = test_plan(&format!("Plan {}", i), "session-1");
            let id = plan.id;
            store.upsert(plan).await.unwrap();
            store.delete(&id).await.unwrap();
        }

        // Journal should have been compacted (stale entries removed)
        assert!(store.journal_len < 150);
    }

    #[tokio::test]
    async fn test_store_backup_on_compaction() {
        // TODO: Test 10
        // Given: a plans.jsonl file with 100+ entries
        // When: compaction is triggered
        // Then:
        //   - a plans.jsonl.bak file is created
        //   - the backup contains the pre-compaction journal
        // Maps to: Infrastructure (data safety)

        let dir = tempdir().unwrap();
        let path = dir.path().join("plans.jsonl");
        let backup_path = dir.path().join("plans.jsonl.bak");
        let mut store = PlanStore::new(path);

        // Create 150 entries to trigger compaction
        for i in 0..150 {
            let plan = test_plan(&format!("Plan {}", i), "session-1");
            let id = plan.id;
            store.upsert(plan).await.unwrap();
            store.delete(&id).await.unwrap();
        }

        // Verify backup was created
        assert!(backup_path.exists());
    }
}
