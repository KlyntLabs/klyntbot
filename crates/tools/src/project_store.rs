//! ProjectStore - Append-only JSONL persistence for project system
//!
//! Mirrors the TodoStore pattern exactly: append-only journal, periodic compaction,
//! lazy loading, and O(1) writes. Projects are organized similarly to todos with
//! status, tags, and filtering capabilities.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::project_types::{Project, ProjectFilter, ProjectPatch};
use common::Result;

/// Compaction threshold: compact when journal has this many more entries than live projects
const COMPACTION_THRESHOLD: usize = 100;

/// A single entry in the append-only journal.
///
/// Tagged enum serialized as `{"_op":"upsert","project":{...}}` or `{"_op":"delete","id":"..."}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "_op")]
enum JournalEntry {
    #[serde(rename = "upsert")]
    Upsert { project: Project },
    #[serde(rename = "delete")]
    Delete { id: String },
}

/// Append-only JSONL-backed project storage with lazy loading and automatic compaction
pub struct ProjectStore {
    file_path: PathBuf,
    /// In-memory index: id -> Project (authoritative state after load)
    index: HashMap<String, Project>,
    /// Ordered list of live project IDs (preserves insertion order)
    order: Vec<String>,
    loaded: bool,
    /// Number of journal entries on disk (including stale/overwritten ones)
    journal_len: usize,
}

impl ProjectStore {
    /// Create a new ProjectStore (does not load from disk yet)
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            index: HashMap::new(),
            order: Vec::new(),
            loaded: false,
            journal_len: 0,
        }
    }

    /// Ensure the store is loaded before any read operation
    async fn ensure_loaded(&mut self) -> Result<()> {
        if !self.loaded {
            self.load().await?;
        }
        Ok(())
    }

    /// Load projects from JSONL file, replaying the journal to build the index
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

            // Parse journal entry
            if let Ok(entry) = serde_json::from_str::<JournalEntry>(line) {
                match entry {
                    JournalEntry::Upsert { project } => {
                        if !index.contains_key(&project.id) {
                            order.push(project.id.clone());
                        }
                        index.insert(project.id.clone(), project);
                    }
                    JournalEntry::Delete { id } => {
                        if index.remove(&id).is_some() {
                            order.retain(|oid| oid != &id);
                        }
                    }
                }
            }
            // Malformed lines are silently skipped
        }

        self.index = index;
        self.order = order;
        self.journal_len = journal_len;
        self.loaded = true;
        Ok(())
    }

    /// Append a single journal entry to the file (O(1) write)
    async fn append_entry(&mut self, entry: &JournalEntry) -> Result<()> {
        // Ensure parent dir exists
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
    ///
    /// Called automatically when stale entries exceed the threshold.
    async fn compact(&mut self) -> Result<()> {
        let mut content = String::with_capacity(self.index.len() * 256);
        for id in &self.order {
            if let Some(project) = self.index.get(id) {
                let entry = JournalEntry::Upsert {
                    project: project.clone(),
                };
                content.push_str(&serde_json::to_string(&entry)?);
                content.push('\n');
            }
        }

        // Ensure parent dir exists
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(&self.file_path, content).await?;
        self.journal_len = self.index.len();
        Ok(())
    }

    /// Check if compaction is needed and run it
    async fn maybe_compact(&mut self) -> Result<()> {
        let stale = self.journal_len.saturating_sub(self.index.len());
        if stale >= COMPACTION_THRESHOLD {
            self.compact().await?;
        }
        Ok(())
    }

    /// Helper: get ordered Vec of live projects (from index, in insertion order)
    fn projects_ordered(&self) -> Vec<&Project> {
        self.order
            .iter()
            .filter_map(|id| self.index.get(id))
            .collect()
    }

    /// Add a new project
    pub async fn add(&mut self, mut project: Project) -> Result<Project> {
        self.ensure_loaded().await?;

        project.updated_at = Utc::now();

        let entry = JournalEntry::Upsert {
            project: project.clone(),
        };
        self.append_entry(&entry).await?;

        self.order.push(project.id.clone());
        self.index.insert(project.id.clone(), project.clone());

        self.maybe_compact().await?;
        Ok(project)
    }

    /// Get a project by ID (O(1) lookup)
    pub async fn get(&mut self, id: &str) -> Result<Option<Project>> {
        self.ensure_loaded().await?;
        Ok(self.index.get(id).cloned())
    }

    /// Update a project with a patch
    pub async fn update(&mut self, id: &str, patch: ProjectPatch) -> Result<Option<Project>> {
        self.ensure_loaded().await?;

        let updated = if let Some(project) = self.index.get_mut(id) {
            if let Some(name) = patch.name {
                project.name = name;
            }
            if let Some(desc) = patch.description {
                project.description = desc;
            }
            if let Some(color) = patch.color {
                project.color = color;
            }
            if let Some(tags) = patch.tags {
                project.tags = tags;
            }
            if let Some(status) = patch.status {
                project.status = status;
            }

            project.updated_at = Utc::now();

            Some(project.clone())
        } else {
            None
        };

        if let Some(ref project) = updated {
            let entry = JournalEntry::Upsert {
                project: project.clone(),
            };
            self.append_entry(&entry).await?;
            self.maybe_compact().await?;
        }

        Ok(updated)
    }

    /// Delete a project by ID
    pub async fn delete(&mut self, id: &str) -> Result<bool> {
        self.ensure_loaded().await?;

        if self.index.remove(id).is_some() {
            self.order.retain(|oid| oid != id);

            let entry = JournalEntry::Delete { id: id.to_string() };
            self.append_entry(&entry).await?;
            self.maybe_compact().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// List projects with optional filters
    pub async fn list(&mut self, filter: &ProjectFilter) -> Result<Vec<Project>> {
        self.ensure_loaded().await?;

        let mut filtered: Vec<Project> = self
            .projects_ordered()
            .into_iter()
            .filter(|p| {
                if let Some(status) = filter.status {
                    if p.status != status {
                        return false;
                    }
                }
                if let Some(tag) = &filter.tag {
                    if !p.tags.contains(tag) {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        if let Some(limit) = filter.limit {
            filtered.truncate(limit);
        }

        Ok(filtered)
    }

    /// Get all projects (for context building, stats, etc.)
    pub async fn all(&mut self) -> Result<Vec<Project>> {
        self.ensure_loaded().await?;
        Ok(self.projects_ordered().into_iter().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_types::{ProjectColor, ProjectStatus};
    use chrono::Utc;
    use tempfile::tempdir;

    fn test_project(name: &str) -> Project {
        Project {
            id: Project::generate_id(),
            name: name.to_string(),
            description: Some("Test project".to_string()),
            color: ProjectColor::Blue,
            tags: vec!["test".to_string()],
            status: ProjectStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_add_and_get() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("projects.jsonl");
        let mut store = ProjectStore::new(path.clone());

        let project = test_project("Test Project");
        let id = project.id.clone();

        let added = store.add(project.clone()).await.unwrap();
        assert_eq!(added.id, id);

        let retrieved = store.get(&id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Test Project");
    }

    #[tokio::test]
    async fn test_update() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("projects.jsonl");
        let mut store = ProjectStore::new(path);

        let project = test_project("Original");
        let id = project.id.clone();
        store.add(project).await.unwrap();

        let patch = ProjectPatch {
            name: Some("Updated".to_string()),
            status: Some(ProjectStatus::Paused),
            ..Default::default()
        };

        let updated = store.update(&id, patch).await.unwrap();
        assert!(updated.is_some());
        let updated = updated.unwrap();
        assert_eq!(updated.name, "Updated");
        assert_eq!(updated.status, ProjectStatus::Paused);
    }

    #[tokio::test]
    async fn test_delete() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("projects.jsonl");
        let mut store = ProjectStore::new(path);

        let project = test_project("To Delete");
        let id = project.id.clone();
        store.add(project).await.unwrap();

        let deleted = store.delete(&id).await.unwrap();
        assert!(deleted);

        let retrieved = store.get(&id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_list_with_filter() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("projects.jsonl");
        let mut store = ProjectStore::new(path);

        let mut p1 = test_project("Active 1");
        p1.status = ProjectStatus::Active;
        store.add(p1).await.unwrap();

        let mut p2 = test_project("Paused 1");
        p2.status = ProjectStatus::Paused;
        store.add(p2).await.unwrap();

        let mut p3 = test_project("Active 2");
        p3.status = ProjectStatus::Active;
        store.add(p3).await.unwrap();

        let filter = ProjectFilter {
            status: Some(ProjectStatus::Active),
            ..Default::default()
        };

        let results = store.list(&filter).await.unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|p| p.status == ProjectStatus::Active));
    }

    #[tokio::test]
    async fn test_persistence() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("projects.jsonl");

        let project = test_project("Persistent");
        let id = project.id.clone();

        // Add in first store instance
        {
            let mut store = ProjectStore::new(path.clone());
            store.add(project).await.unwrap();
        }

        // Retrieve in second store instance
        {
            let mut store = ProjectStore::new(path.clone());
            let retrieved = store.get(&id).await.unwrap();
            assert!(retrieved.is_some());
            assert_eq!(retrieved.unwrap().name, "Persistent");
        }
    }

    #[tokio::test]
    async fn test_compaction() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("projects.jsonl");
        let mut store = ProjectStore::new(path.clone());

        // Add and delete many projects to trigger compaction
        for i in 0..150 {
            let project = test_project(&format!("Project {}", i));
            let id = project.id.clone();
            store.add(project).await.unwrap();
            store.delete(&id).await.unwrap();
        }

        // Journal should have been compacted
        assert!(store.journal_len < 150);
    }
}
