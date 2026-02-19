//! GoalStore - Append-only JSONL persistence for goal engine
//!
//! Mirrors the ProjectStore pattern: append-only journal, periodic compaction,
//! lazy loading, and O(1) writes.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::types::{Goal, GoalProgress, GoalStatus, Metric};
use common::Result;
use std::str::FromStr;

/// Compaction threshold: compact when journal has this many more entries than live goals
const COMPACTION_THRESHOLD: usize = 100;

/// A single entry in the append-only journal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "_op")]
enum JournalEntry {
    #[serde(rename = "upsert")]
    Upsert { goal: Goal },
    #[serde(rename = "delete")]
    Delete { id: Uuid },
}

/// Append-only JSONL-backed goal storage with lazy loading and automatic compaction.
/// Optionally backed by a SQL repository when constructed via `from_repo()`.
pub struct GoalStore {
    file_path: PathBuf,
    /// In-memory index: id -> Goal (authoritative state after load)
    index: HashMap<Uuid, Goal>,
    /// Ordered list of live goal IDs (preserves insertion order)
    order: Vec<Uuid>,
    loaded: bool,
    /// Number of journal entries on disk (including stale/overwritten ones)
    journal_len: usize,
    /// Optional SQL backend — when present, all CRUD is delegated to SQL.
    sql_repo: Option<storage::GoalRepo>,
}

impl GoalStore {
    /// Create a new GoalStore backed by JSONL (does not load from disk yet).
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

    /// Create a GoalStore backed by a SQL repository.
    pub fn from_repo(repo: storage::GoalRepo) -> Self {
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

    fn goal_to_row(goal: &Goal) -> storage::GoalRow {
        storage::GoalRow {
            id: goal.id,
            title: goal.title.clone(),
            description: goal.description.clone(),
            status: goal.status.to_string(),
            priority: goal.priority as i16,
            target_date: goal.target_date,
            created_at: goal.created_at,
            updated_at: goal.updated_at,
            metrics: serde_json::to_value(&goal.metrics).unwrap_or_default(),
            metadata: serde_json::to_value(&goal.metadata).unwrap_or_default(),
        }
    }

    fn row_to_goal(row: storage::GoalRow, linked_project_ids: Vec<Uuid>) -> Goal {
        Goal {
            id: row.id,
            title: row.title,
            description: row.description,
            status: GoalStatus::from_str(&row.status).unwrap_or_default(),
            priority: row.priority as u8,
            target_date: row.target_date,
            created_at: row.created_at,
            updated_at: row.updated_at,
            metrics: serde_json::from_value::<Vec<Metric>>(row.metrics).unwrap_or_default(),
            linked_project_ids,
            metadata: serde_json::from_value(row.metadata).unwrap_or_default(),
        }
    }

    /// Ensure the store is loaded before any read operation.
    async fn ensure_loaded(&mut self) -> Result<()> {
        if !self.loaded {
            self.load().await?;
        }
        Ok(())
    }

    /// Load goals from JSONL file, replaying the journal to build the index.
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
                    JournalEntry::Upsert { goal } => {
                        if !index.contains_key(&goal.id) {
                            order.push(goal.id);
                        }
                        index.insert(goal.id, goal);
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
    /// Creates a `.jsonl.bak` backup before overwriting.
    async fn compact(&mut self) -> Result<()> {
        // Create backup before compaction
        let backup_path = self.file_path.with_extension("jsonl.bak");
        if self.file_path.exists() {
            fs::copy(&self.file_path, &backup_path).await?;
        }

        let mut content = String::with_capacity(self.index.len() * 256);
        for id in &self.order {
            if let Some(goal) = self.index.get(id) {
                let entry = JournalEntry::Upsert { goal: goal.clone() };
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

    /// Helper: get ordered Vec of live goals.
    fn goals_ordered(&self) -> Vec<&Goal> {
        self.order
            .iter()
            .filter_map(|id| self.index.get(id))
            .collect()
    }

    /// Add a new goal.
    pub async fn add(&mut self, mut goal: Goal) -> Result<Goal> {
        if let Some(repo) = &self.sql_repo {
            goal.updated_at = Utc::now();
            let row = Self::goal_to_row(&goal);
            repo.create(&row)
                .await?;
            // Link projects
            for pid in &goal.linked_project_ids {
                repo.link_project(goal.id, &pid.to_string())
                    .await?;
            }
            return Ok(goal);
        }

        self.ensure_loaded().await?;

        goal.updated_at = Utc::now();

        let entry = JournalEntry::Upsert { goal: goal.clone() };
        self.append_entry(&entry).await?;

        self.order.push(goal.id);
        self.index.insert(goal.id, goal.clone());

        self.maybe_compact().await?;
        Ok(goal)
    }

    /// Get a goal by ID (O(1) lookup).
    pub async fn get(&mut self, id: &Uuid) -> Result<Option<Goal>> {
        if let Some(repo) = &self.sql_repo {
            return match repo.get(*id).await {
                Ok(row) => {
                    let links = repo
                        .get_project_links(*id)
                        .await?;
                    let project_ids = links
                        .into_iter()
                        .filter_map(|l| Uuid::parse_str(&l.project_id).ok())
                        .collect();
                    Ok(Some(Self::row_to_goal(row, project_ids)))
                }
                Err(storage::StorageError::NotFound(_)) => Ok(None),
                Err(e) => Err(e.into()),
            };
        }

        self.ensure_loaded().await?;
        Ok(self.index.get(id).cloned())
    }

    /// Update a goal in place. Validates status transitions.
    pub async fn update(&mut self, goal: Goal) -> Result<Option<Goal>> {
        if let Some(repo) = &self.sql_repo {
            // Validate status transition
            match repo.get(goal.id).await {
                Ok(existing) => {
                    let old_status =
                        GoalStatus::from_str(&existing.status).unwrap_or_default();
                    GoalStatus::validate_transition(&old_status, &goal.status)?;
                }
                Err(storage::StorageError::NotFound(_)) => return Ok(None),
                Err(e) => return Err(e.into()),
            }

            let mut updated = goal;
            updated.updated_at = Utc::now();
            let row = Self::goal_to_row(&updated);
            match repo.update(&row).await {
                Ok(_) => {
                    // Sync project links: clear existing and re-link
                    let existing = repo
                        .get_project_links(updated.id)
                        .await?;
                    for link in &existing {
                        let _ = repo.unlink_project(updated.id, &link.project_id).await;
                    }
                    for pid in &updated.linked_project_ids {
                        repo.link_project(updated.id, &pid.to_string())
                            .await?;
                    }
                    Ok(Some(updated))
                }
                Err(storage::StorageError::NotFound(_)) => Ok(None),
                Err(e) => Err(e.into()),
            }
        } else {
            self.ensure_loaded().await?;

            if !self.index.contains_key(&goal.id) {
                return Ok(None);
            }

            // Validate status transition (JSONL path)
            if let Some(existing) = self.index.get(&goal.id) {
                GoalStatus::validate_transition(&existing.status, &goal.status)?;
            }

            let mut updated = goal;
            updated.updated_at = Utc::now();

            let entry = JournalEntry::Upsert {
                goal: updated.clone(),
            };
            self.append_entry(&entry).await?;
            self.index.insert(updated.id, updated.clone());

            self.maybe_compact().await?;
            Ok(Some(updated))
        }
    }

    /// Delete a goal by ID.
    pub async fn delete(&mut self, id: &Uuid) -> Result<bool> {
        if let Some(repo) = &self.sql_repo {
            return repo
                .delete(*id)
                .await
                .map_err(common::KlyntbotError::from);
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

    /// List all goals, optionally filtered by status.
    pub async fn list(&mut self, status: Option<GoalStatus>) -> Result<Vec<Goal>> {
        if let Some(repo) = &self.sql_repo {
            let status_str = status.as_ref().map(|s| s.to_string());
            let rows = repo
                .list(status_str.as_deref())
                .await?;
            let mut goals = Vec::with_capacity(rows.len());
            for row in rows {
                let links = repo
                    .get_project_links(row.id)
                    .await?;
                let project_ids = links
                    .into_iter()
                    .filter_map(|l| Uuid::parse_str(&l.project_id).ok())
                    .collect();
                goals.push(Self::row_to_goal(row, project_ids));
            }
            return Ok(goals);
        }

        self.ensure_loaded().await?;

        let filtered: Vec<Goal> = self
            .goals_ordered()
            .into_iter()
            .filter(|g| {
                if let Some(ref s) = status {
                    &g.status == s
                } else {
                    true
                }
            })
            .cloned()
            .collect();

        Ok(filtered)
    }

    /// Calculate progress for a goal based on its metrics.
    ///
    /// Returns `None` if the goal doesn't exist.
    /// Completion percentage is the average of all metric progress percentages,
    /// or 0.0 if no metrics are defined.
    pub async fn calculate_progress(&mut self, id: &Uuid) -> Result<Option<GoalProgress>> {
        if self.sql_repo.is_some() {
            // Delegate to get() which handles SQL, then compute progress.
            let goal = match self.get(id).await? {
                Some(g) => g,
                None => return Ok(None),
            };
            return Ok(Some(Self::compute_progress(id, &goal)));
        }

        self.ensure_loaded().await?;

        let goal = match self.index.get(id) {
            Some(g) => g,
            None => return Ok(None),
        };

        Ok(Some(Self::compute_progress(id, goal)))
    }

    /// Compute a GoalProgress from a goal reference (shared by JSONL and SQL paths).
    fn compute_progress(id: &Uuid, goal: &Goal) -> GoalProgress {
        let completion = if goal.metrics.is_empty() {
            0.0
        } else {
            let sum: f64 = goal.metrics.iter().map(|m| m.progress_percentage()).sum();
            sum / goal.metrics.len() as f64
        };

        let summary = if goal.metrics.is_empty() {
            "No metrics defined".to_string()
        } else {
            format!(
                "{:.0}% complete across {} metric(s)",
                completion,
                goal.metrics.len()
            )
        };

        GoalProgress {
            goal_id: *id,
            completion_percentage: completion,
            metrics: goal.metrics.clone(),
            summary,
        }
    }

    /// Get all goals.
    pub async fn all(&mut self) -> Result<Vec<Goal>> {
        if self.sql_repo.is_some() {
            return self.list(None).await;
        }

        self.ensure_loaded().await?;
        Ok(self.goals_ordered().into_iter().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Metric;
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn test_goal(title: &str) -> Goal {
        let now = Utc::now();
        Goal {
            id: Uuid::new_v4(),
            title: title.to_string(),
            description: format!("Description for {}", title),
            status: GoalStatus::Active,
            priority: 2,
            target_date: None,
            created_at: now,
            updated_at: now,
            metrics: vec![],
            linked_project_ids: vec![],
            metadata: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_store_creation() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("goals.jsonl");
        let mut store = GoalStore::new(path);

        let goals = store.all().await.unwrap();
        assert!(goals.is_empty());
    }

    #[tokio::test]
    async fn test_add_and_get() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("goals.jsonl");
        let mut store = GoalStore::new(path);

        let goal = test_goal("Learn Rust");
        let id = goal.id;

        let added = store.add(goal).await.unwrap();
        assert_eq!(added.id, id);

        let retrieved = store.get(&id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title, "Learn Rust");
    }

    #[tokio::test]
    async fn test_update() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("goals.jsonl");
        let mut store = GoalStore::new(path);

        let goal = test_goal("Original Title");
        let id = goal.id;
        store.add(goal).await.unwrap();

        let mut to_update = store.get(&id).await.unwrap().unwrap();
        to_update.title = "Updated Title".to_string();
        to_update.status = GoalStatus::Paused;

        let updated = store.update(to_update).await.unwrap();
        assert!(updated.is_some());
        let updated = updated.unwrap();
        assert_eq!(updated.title, "Updated Title");
        assert_eq!(updated.status, GoalStatus::Paused);
    }

    #[tokio::test]
    async fn test_delete() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("goals.jsonl");
        let mut store = GoalStore::new(path);

        let goal = test_goal("To Delete");
        let id = goal.id;
        store.add(goal).await.unwrap();

        let deleted = store.delete(&id).await.unwrap();
        assert!(deleted);

        let retrieved = store.get(&id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_list_with_status_filter() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("goals.jsonl");
        let mut store = GoalStore::new(path);

        let mut g1 = test_goal("Active Goal");
        g1.status = GoalStatus::Active;
        store.add(g1).await.unwrap();

        let mut g2 = test_goal("Paused Goal");
        g2.status = GoalStatus::Paused;
        store.add(g2).await.unwrap();

        let mut g3 = test_goal("Another Active");
        g3.status = GoalStatus::Active;
        store.add(g3).await.unwrap();

        let active = store.list(Some(GoalStatus::Active)).await.unwrap();
        assert_eq!(active.len(), 2);
        assert!(active.iter().all(|g| g.status == GoalStatus::Active));

        let all = store.list(None).await.unwrap();
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn test_persistence_across_instances() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("goals.jsonl");

        let goal = test_goal("Persistent Goal");
        let id = goal.id;

        {
            let mut store = GoalStore::new(path.clone());
            store.add(goal).await.unwrap();
        }

        {
            let mut store = GoalStore::new(path);
            let retrieved = store.get(&id).await.unwrap();
            assert!(retrieved.is_some());
            assert_eq!(retrieved.unwrap().title, "Persistent Goal");
        }
    }

    #[tokio::test]
    async fn test_compaction() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("goals.jsonl");
        let mut store = GoalStore::new(path);

        for i in 0..150 {
            let goal = test_goal(&format!("Goal {}", i));
            let id = goal.id;
            store.add(goal).await.unwrap();
            store.delete(&id).await.unwrap();
        }

        // Journal should have been compacted (stale entries removed)
        assert!(store.journal_len < 150);

        // Verify backup was created during compaction
        let backup_path = dir.path().join("goals.jsonl.bak");
        assert!(backup_path.exists());
    }

    #[tokio::test]
    async fn test_calculate_progress_no_metrics() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("goals.jsonl");
        let mut store = GoalStore::new(path);

        let goal = test_goal("No Metrics");
        let id = goal.id;
        store.add(goal).await.unwrap();

        let progress = store.calculate_progress(&id).await.unwrap();
        assert!(progress.is_some());
        let progress = progress.unwrap();
        assert_eq!(progress.goal_id, id);
        assert_eq!(progress.completion_percentage, 0.0);
        assert!(progress.metrics.is_empty());
    }

    #[tokio::test]
    async fn test_calculate_progress_with_metrics() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("goals.jsonl");
        let mut store = GoalStore::new(path);

        let mut goal = test_goal("With Metrics");
        goal.metrics = vec![
            Metric {
                name: "Tasks".to_string(),
                current: 5.0,
                target: 10.0,
                unit: "tasks".to_string(),
            },
            Metric {
                name: "Docs".to_string(),
                current: 8.0,
                target: 8.0,
                unit: "pages".to_string(),
            },
        ];
        let id = goal.id;
        store.add(goal).await.unwrap();

        let progress = store.calculate_progress(&id).await.unwrap();
        assert!(progress.is_some());
        let progress = progress.unwrap();
        assert_eq!(progress.goal_id, id);
        // (50 + 100) / 2 = 75
        assert!((progress.completion_percentage - 75.0).abs() < f64::EPSILON);
        assert_eq!(progress.metrics.len(), 2);
    }

    #[tokio::test]
    async fn test_calculate_progress_nonexistent_goal() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("goals.jsonl");
        let mut store = GoalStore::new(path);

        let progress = store.calculate_progress(&Uuid::new_v4()).await.unwrap();
        assert!(progress.is_none());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_goal() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("goals.jsonl");
        let mut store = GoalStore::new(path);

        let deleted = store.delete(&Uuid::new_v4()).await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_goal_with_metrics_persistence() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("goals.jsonl");

        let mut goal = test_goal("Metrics Goal");
        goal.metrics = vec![Metric {
            name: "Tasks".to_string(),
            current: 3.0,
            target: 10.0,
            unit: "tasks".to_string(),
        }];
        let id = goal.id;

        {
            let mut store = GoalStore::new(path.clone());
            store.add(goal).await.unwrap();
        }

        {
            let mut store = GoalStore::new(path);
            let retrieved = store.get(&id).await.unwrap().unwrap();
            assert_eq!(retrieved.metrics.len(), 1);
            assert_eq!(retrieved.metrics[0].name, "Tasks");
            assert_eq!(retrieved.metrics[0].current, 3.0);
        }
    }

    #[tokio::test]
    async fn test_update_rejects_invalid_status_transition() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("goals.jsonl");
        let mut store = GoalStore::new(path);

        let mut goal = test_goal("Achieved Goal");
        goal.status = GoalStatus::Active;
        let id = goal.id;
        store.add(goal).await.unwrap();

        // Active → Achieved is valid
        let mut to_achieve = store.get(&id).await.unwrap().unwrap();
        to_achieve.status = GoalStatus::Achieved;
        store.update(to_achieve).await.unwrap();

        // Achieved → Active is invalid (final state)
        let mut to_reactivate = store.get(&id).await.unwrap().unwrap();
        to_reactivate.status = GoalStatus::Active;
        let result = store.update(to_reactivate).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_allows_valid_status_transition() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("goals.jsonl");
        let mut store = GoalStore::new(path);

        let goal = test_goal("Pausable Goal");
        let id = goal.id;
        store.add(goal).await.unwrap();

        // Active → Paused → Active → Abandoned
        let mut g = store.get(&id).await.unwrap().unwrap();
        g.status = GoalStatus::Paused;
        store.update(g).await.unwrap();

        let mut g = store.get(&id).await.unwrap().unwrap();
        g.status = GoalStatus::Active;
        store.update(g).await.unwrap();

        let mut g = store.get(&id).await.unwrap().unwrap();
        g.status = GoalStatus::Abandoned;
        store.update(g).await.unwrap();

        // Abandoned is final
        let mut g = store.get(&id).await.unwrap().unwrap();
        g.status = GoalStatus::Active;
        assert!(store.update(g).await.is_err());
    }
}
