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

use crate::types::{BacktrackEntry, Plan, PlanStatus, PlanStep, StepStatus};
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

/// Borrowing version of JournalEntry — used by persist_latest to serialize
/// without cloning the Plan. Produces identical JSON to JournalEntry::Upsert.
#[derive(Serialize)]
#[serde(tag = "_op")]
enum JournalEntryRef<'a> {
    #[serde(rename = "upsert")]
    Upsert { plan: &'a Plan },
}

/// Append-only JSONL-backed plan storage with lazy loading and automatic compaction.
/// Optionally backed by a SQL repository when constructed via `from_repo()`.
pub struct PlanStore {
    file_path: PathBuf,
    /// In-memory index: id -> Plan (authoritative state after load)
    index: HashMap<Uuid, Plan>,
    /// Ordered list of live plan IDs (preserves insertion order)
    order: Vec<Uuid>,
    loaded: bool,
    /// Number of journal entries on disk (including stale/overwritten ones)
    pub journal_len: usize,
    /// Optional SQL backend — when present, all CRUD is delegated to SQL.
    sql_repo: Option<storage::PlanRepo>,
}

impl PlanStore {
    /// Create a new PlanStore backed by JSONL (does not load from disk yet).
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            index: HashMap::new(),
            order: Vec::new(),
            loaded: false,
            journal_len: 0,
            sql_repo: None,
        }
    }

    /// Create a PlanStore backed by a SQL repository.
    pub fn from_repo(repo: storage::PlanRepo) -> Self {
        Self {
            file_path: PathBuf::from("/dev/null"),
            index: HashMap::new(),
            order: Vec::new(),
            loaded: true,
            journal_len: 0,
            sql_repo: Some(repo),
        }
    }

    // ── SQL ↔ Domain conversion helpers ─────────────────────────

    fn plan_status_to_str(status: &PlanStatus) -> &'static str {
        match status {
            PlanStatus::Draft => "draft",
            PlanStatus::Approved => "approved",
            PlanStatus::Executing => "executing",
            PlanStatus::Completed => "completed",
            PlanStatus::Failed => "failed",
            PlanStatus::Abandoned => "abandoned",
        }
    }

    fn str_to_plan_status(s: &str) -> PlanStatus {
        match s.to_lowercase().as_str() {
            "draft" => PlanStatus::Draft,
            "approved" => PlanStatus::Approved,
            "executing" => PlanStatus::Executing,
            "completed" => PlanStatus::Completed,
            "failed" => PlanStatus::Failed,
            "abandoned" => PlanStatus::Abandoned,
            _ => PlanStatus::Draft,
        }
    }

    fn step_status_to_str(status: &StepStatus) -> &'static str {
        match status {
            StepStatus::Pending => "pending",
            StepStatus::Executing => "executing",
            StepStatus::Completed => "completed",
            StepStatus::Failed => "failed",
            StepStatus::Skipped => "skipped",
        }
    }

    fn str_to_step_status(s: &str) -> StepStatus {
        match s.to_lowercase().as_str() {
            "pending" => StepStatus::Pending,
            "executing" => StepStatus::Executing,
            "completed" => StepStatus::Completed,
            "failed" => StepStatus::Failed,
            "skipped" => StepStatus::Skipped,
            _ => StepStatus::Pending,
        }
    }

    fn plan_to_row(plan: &Plan) -> storage::PlanRow {
        storage::PlanRow {
            id: plan.id,
            session_key: plan.session_key.clone(),
            goal_id: plan.goal_id,
            title: plan.title.clone(),
            description: plan.description.clone(),
            status: Self::plan_status_to_str(&plan.status).to_string(),
            current_step_index: plan.current_step_index as i32,
            iteration_limit: plan.iteration_limit as i32,
            backtrack_history: serde_json::to_value(&plan.backtrack_history).unwrap_or_default(),
            created_at: plan.created_at,
            updated_at: plan.updated_at,
            completed_at: plan.completed_at,
        }
    }

    fn step_to_row(step: &PlanStep, plan_id: Uuid) -> storage::PlanStepRow {
        storage::PlanStepRow {
            id: step.id,
            plan_id,
            step_index: step.index as i32,
            description: step.description.clone(),
            reasoning: step.reasoning.clone(),
            expected_tools: step.expected_tools.clone(),
            status: Self::step_status_to_str(&step.status).to_string(),
            attempt_count: step.attempt_count as i16,
            max_attempts: step.max_attempts as i16,
            result: step.result.clone(),
            started_at: step.started_at,
            completed_at: step.completed_at,
        }
    }

    fn row_to_plan(row: storage::PlanRow, step_rows: Vec<storage::PlanStepRow>) -> Plan {
        let steps: Vec<PlanStep> = step_rows
            .into_iter()
            .map(|sr| PlanStep {
                id: sr.id,
                index: sr.step_index as usize,
                description: sr.description,
                reasoning: sr.reasoning,
                expected_tools: sr.expected_tools,
                status: Self::str_to_step_status(&sr.status),
                attempt_count: sr.attempt_count as u8,
                max_attempts: sr.max_attempts as u8,
                result: sr.result,
                started_at: sr.started_at,
                completed_at: sr.completed_at,
            })
            .collect();

        Plan {
            id: row.id,
            session_key: row.session_key,
            goal_id: row.goal_id,
            title: row.title,
            description: row.description,
            status: Self::str_to_plan_status(&row.status),
            steps,
            current_step_index: row.current_step_index as usize,
            iteration_limit: row.iteration_limit as usize,
            backtrack_history: serde_json::from_value::<Vec<BacktrackEntry>>(row.backtrack_history)
                .unwrap_or_default(),
            created_at: row.created_at,
            updated_at: row.updated_at,
            completed_at: row.completed_at,
        }
    }

    /// Helper: load a plan from SQL by ID, combining the plan row and its step rows.
    async fn sql_get(&self, repo: &storage::PlanRepo, id: Uuid) -> Result<Option<Plan>> {
        match repo.get(id).await {
            Ok(row) => {
                let steps = repo
                    .get_steps(id)
                    .await
                    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
                Ok(Some(Self::row_to_plan(row, steps)))
            }
            Err(storage::StorageError::NotFound(_)) => Ok(None),
            Err(e) => Err(common::KlyntbotError::Storage(e.to_string())),
        }
    }

    /// Helper: upsert a plan + all steps into SQL.
    async fn sql_upsert(&self, repo: &storage::PlanRepo, plan: &Plan) -> Result<()> {
        let row = Self::plan_to_row(plan);

        // Try create, fall back to update
        match repo.create(&row).await {
            Ok(_) => {}
            Err(storage::StorageError::Sqlx(_)) => {
                // Likely duplicate key — try update
                repo.update(&row)
                    .await
                    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
            }
            Err(e) => return Err(common::KlyntbotError::Storage(e.to_string())),
        }

        // Upsert steps: get existing, add or update each
        let existing_steps = repo
            .get_steps(plan.id)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        let existing_ids: std::collections::HashSet<Uuid> =
            existing_steps.iter().map(|s| s.id).collect();

        for step in &plan.steps {
            let step_row = Self::step_to_row(step, plan.id);
            if existing_ids.contains(&step.id) {
                repo.update_step(&step_row)
                    .await
                    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
            } else {
                repo.add_step(&step_row)
                    .await
                    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
            }
        }

        Ok(())
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
        plan.updated_at = Utc::now();

        if let Some(repo) = &self.sql_repo {
            self.sql_upsert(repo, &plan).await?;
            return Ok(plan);
        }

        self.ensure_loaded().await?;

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
        if let Some(repo) = &self.sql_repo {
            return self.sql_get(repo, *id).await;
        }

        self.ensure_loaded().await?;
        Ok(self.index.get(id).cloned())
    }

    /// Get a mutable reference to a plan for in-place mutation (zero-copy).
    ///
    /// Returns `None` if the plan does not exist. After mutating via the returned
    /// reference, call `persist_latest(id)` to write the updated state to the journal.
    ///
    /// For the SQL path: loads the plan into the in-memory index so callers can
    /// mutate it. Call `persist_latest(id)` afterwards to write changes back to SQL.
    pub async fn get_mut(&mut self, id: &Uuid) -> Result<Option<&mut Plan>> {
        if let Some(repo) = &self.sql_repo {
            // Load from SQL into in-memory index if not already cached
            if !self.index.contains_key(id) {
                match repo.get(*id).await {
                    Ok(row) => {
                        let steps = repo
                            .get_steps(*id)
                            .await
                            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
                        let plan = Self::row_to_plan(row, steps);
                        self.index.insert(*id, plan);
                    }
                    Err(storage::StorageError::NotFound(_)) => return Ok(None),
                    Err(e) => return Err(common::KlyntbotError::Storage(e.to_string())),
                }
            }
            return Ok(self.index.get_mut(id));
        }

        self.ensure_loaded().await?;
        Ok(self.index.get_mut(id))
    }

    /// Persist the current in-memory plan state.
    ///
    /// For JSONL: appends to journal without cloning (uses `JournalEntryRef`).
    /// For SQL: writes the in-memory state back to the database.
    /// If the plan does not exist in memory, this is a no-op.
    pub async fn persist_latest(&mut self, id: &Uuid) -> Result<()> {
        if let Some(repo) = &self.sql_repo {
            if let Some(plan) = self.index.get(id) {
                let plan_clone = plan.clone();
                self.sql_upsert(repo, &plan_clone).await?;
            }
            return Ok(());
        }

        self.ensure_loaded().await?;

        if let Some(plan) = self.index.get(id) {
            let entry = JournalEntryRef::Upsert { plan };
            let mut line = serde_json::to_string(&entry)?;
            line.push('\n');

            if let Some(parent) = self.file_path.parent() {
                fs::create_dir_all(parent).await?;
            }

            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.file_path)
                .await?;
            file.write_all(line.as_bytes()).await?;
            file.flush().await?;

            self.journal_len += 1;
            self.maybe_compact().await?;
        }

        Ok(())
    }

    /// Get the most recent active plan for a session (by updated_at timestamp).
    /// Returns Draft, Approved, or Executing plans only, sorted by most recent first.
    pub async fn get_active_plan(&mut self, session_key: &str) -> Result<Option<Plan>> {
        if let Some(repo) = &self.sql_repo {
            // Try each active status and combine
            let mut candidates = Vec::new();
            for status in &["draft", "approved", "executing"] {
                let rows = repo
                    .list(Some(status), Some(session_key), None)
                    .await
                    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
                for row in rows {
                    let steps = repo
                        .get_steps(row.id)
                        .await
                        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
                    candidates.push(Self::row_to_plan(row, steps));
                }
            }
            candidates.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            return Ok(candidates.into_iter().next());
        }

        self.ensure_loaded().await?;

        let mut active_plans: Vec<&Plan> = self
            .index
            .values()
            .filter(|p| {
                p.session_key == session_key
                    && (p.status == PlanStatus::Draft
                        || p.status == PlanStatus::Approved
                        || p.status == PlanStatus::Executing)
            })
            .collect();

        // Sort by updated_at descending (most recent first)
        active_plans.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(active_plans.first().map(|p| (*p).clone()))
    }

    /// List all plans in insertion order.
    pub async fn all(&mut self) -> Result<Vec<Plan>> {
        if let Some(repo) = &self.sql_repo {
            let rows = repo
                .list(None, None, None)
                .await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
            let mut plans = Vec::with_capacity(rows.len());
            for row in rows {
                let steps = repo
                    .get_steps(row.id)
                    .await
                    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
                plans.push(Self::row_to_plan(row, steps));
            }
            return Ok(plans);
        }

        self.ensure_loaded().await?;
        Ok(self
            .order
            .iter()
            .filter_map(|id| self.index.get(id))
            .cloned()
            .collect())
    }

    /// List plans filtered by status.
    pub async fn list_by_status(&mut self, status: &PlanStatus) -> Result<Vec<Plan>> {
        if let Some(repo) = &self.sql_repo {
            let status_str = Self::plan_status_to_str(status);
            let rows = repo
                .list(Some(status_str), None, None)
                .await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
            let mut plans = Vec::with_capacity(rows.len());
            for row in rows {
                let steps = repo
                    .get_steps(row.id)
                    .await
                    .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
                plans.push(Self::row_to_plan(row, steps));
            }
            return Ok(plans);
        }

        self.ensure_loaded().await?;
        Ok(self
            .order
            .iter()
            .filter_map(|id| self.index.get(id))
            .filter(|p| &p.status == status)
            .cloned()
            .collect())
    }

    /// Delete a plan by ID.
    pub async fn delete(&mut self, id: &Uuid) -> Result<bool> {
        if let Some(repo) = &self.sql_repo {
            self.index.remove(id);
            return repo
                .delete(*id)
                .await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()));
        }

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
