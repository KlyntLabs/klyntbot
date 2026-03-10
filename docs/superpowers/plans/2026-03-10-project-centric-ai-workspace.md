# Project-Centric AI Workspace Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform Klyntbot's project management into a unified AI workspace where projects are living dashboards with cross-feature linking, AI context (instructions, sources, memory), and a 3-panel timeline UI.

**Architecture:** Edit existing SQL migrations directly (no backward compat), add new repos/handlers following established patterns (AppCore → Tauri command → emit_updates), extend cognitive memory with project scoping, add ProjectContextSource to the context engine, and rewrite the ProjectDetailPage as a 3-panel dashboard.

**Tech Stack:** Rust (SQLite, sqlx, LanceDB, async_trait), React 19 (TypeScript, Tailwind v4, Tiptap), Tauri 2 IPC.

**Spec:** `docs/superpowers/specs/2026-03-10-project-centric-ai-workspace-design.md`

---

## Chunk 1: Schema & Domain Types (Phase 1a)

### Task 1: Edit SQL Migrations — Add New Columns and Tables

**Files:**
- Modify: `crates/storage/migrations/001_initial.sql:22-32` (projects table)
- Modify: `crates/storage/migrations/001_initial.sql:158-163` (sessions table)
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql:3-19` (semantic_facts)
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql:25-36` (episodic_memories)
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql:41-51` (procedural_rules)

- [ ] **Step 1: Add new columns to `projects` table in `001_initial.sql`**

Before the closing parenthesis of the CREATE TABLE block (after `updated_at` on line 31), add:

```sql
    instructions    TEXT,
    ai_personality  TEXT,
    user_role       TEXT,
    start_date      TEXT,
    target_end_date TEXT,
    settings        TEXT
```

- [ ] **Step 2: Add new columns to `sessions` table in `001_initial.sql`**

After existing columns (around line 162), add:

```sql
    project_id        TEXT REFERENCES projects(id),
    conversation_type TEXT DEFAULT 'general',
    pinned            INTEGER DEFAULT 0
```

- [ ] **Step 3: Add `entity_links` table to `001_initial.sql`**

Append after the existing tables:

```sql
CREATE TABLE entity_links (
    id          TEXT PRIMARY KEY,
    source_kind TEXT NOT NULL,
    source_id   TEXT NOT NULL,
    target_kind TEXT NOT NULL,
    target_id   TEXT NOT NULL,
    link_type   TEXT NOT NULL DEFAULT 'related',
    metadata    TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    UNIQUE(source_kind, source_id, target_kind, target_id, link_type)
);

CREATE INDEX idx_entity_links_source ON entity_links(source_kind, source_id);
CREATE INDEX idx_entity_links_target ON entity_links(target_kind, target_id);
```

- [ ] **Step 4: Add `project_sources` table to `001_initial.sql`**

```sql
CREATE TABLE project_sources (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_type TEXT NOT NULL,
    title       TEXT NOT NULL,
    content     TEXT,
    url         TEXT,
    file_path   TEXT,
    embedding_id TEXT,
    metadata    TEXT,
    tags        TEXT DEFAULT '[]',
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_project_sources_project ON project_sources(project_id);
```

- [ ] **Step 5: Add `project_id` and `memory_type` to cognitive tables**

In `crates/cognitive/migrations/001_cognitive_tables.sql`:

Add to `semantic_facts` (after `access_count` column):
```sql
    project_id    TEXT,  -- logical FK to projects.id (not enforced, separate migration file)
    memory_type   TEXT DEFAULT 'fact'
```

Add to `episodic_memories` (after `access_count`):
```sql
    project_id    TEXT   -- logical FK to projects.id (not enforced, separate migration file)
```

Add to `procedural_rules` (after `active`):
```sql
    project_id    TEXT   -- logical FK to projects.id (not enforced, separate migration file)
```

> **Note:** These cognitive tables live in a separate database (`crates/cognitive/migrations/`), so SQL REFERENCES constraints to `projects.id` cannot be enforced at the DB level. The `project_id` columns are logical foreign keys only.

Add indexes at the end of the file:
```sql
CREATE INDEX idx_semantic_facts_project ON semantic_facts(project_id);
CREATE INDEX idx_episodic_memories_project ON episodic_memories(project_id);
CREATE INDEX idx_procedural_rules_project ON procedural_rules(project_id);
```

- [ ] **Step 6: Verify migrations compile**

Run: `cargo build -p storage -p cognitive 2>&1 | head -30`
Expected: Build success (migrations are embedded via `include_str!`)

- [ ] **Step 7: Commit**

```bash
git add crates/storage/migrations/001_initial.sql crates/cognitive/migrations/001_cognitive_tables.sql
git commit -m "feat(storage): add entity_links, project_sources tables and project-scoped columns"
```

---

### Task 2: Define Domain Types — EntityLink, ProjectSource, Summary Types

**Files:**
- Create: `crates/storage/src/rows/entity_link.rs`
- Create: `crates/storage/src/rows/project_source.rs`
- Modify: `crates/storage/src/rows/mod.rs` (add mod declarations)

- [ ] **Step 1: Create `entity_link.rs` row type**

```rust
// crates/storage/src/rows/entity_link.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct EntityLinkRow {
    pub id: String,
    pub source_kind: String,
    pub source_id: String,
    pub target_kind: String,
    pub target_id: String,
    pub link_type: String,
    pub metadata: Option<String>,
    pub created_at: DateTime<Utc>,
}
```

- [ ] **Step 2: Create `project_source.rs` row type**

```rust
// crates/storage/src/rows/project_source.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSourceRow {
    pub id: String,
    pub project_id: String,
    pub source_type: String,
    pub title: String,
    pub content: Option<String>,
    pub url: Option<String>,
    pub file_path: Option<String>,
    pub embedding_id: Option<String>,
    pub metadata: Option<String>,
    pub tags: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

- [ ] **Step 3: Register new row modules**

In `crates/storage/src/rows/mod.rs`, add:
```rust
pub mod entity_link;
pub mod project_source;

pub use entity_link::EntityLinkRow;
pub use project_source::ProjectSourceRow;
```

- [ ] **Step 4: Verify build**

Run: `cargo build -p storage 2>&1 | head -20`
Expected: Build success

- [ ] **Step 5: Commit**

```bash
git add crates/storage/src/rows/entity_link.rs crates/storage/src/rows/project_source.rs crates/storage/src/rows/mod.rs
git commit -m "feat(storage): add EntityLinkRow and ProjectSourceRow types"
```

---

### Task 3: Update Existing Row Structs — ProjectRow, SessionRow, Cognitive Types

**Files:**
- Modify: `crates/storage/src/rows/project.rs:10-22` (ProjectRow)
- Modify: `crates/storage/src/rows/session.rs:10-15` (SessionRow)
- Modify: `crates/cognitive/src/types.rs:17-35` (SemanticFact)
- Modify: `crates/cognitive/src/types.rs:39-50` (EpisodicMemory)
- Modify: `crates/cognitive/src/types.rs:54-64` (ProceduralRule)

- [ ] **Step 1: Add new fields to `ProjectRow`**

In `crates/storage/src/rows/project.rs`, add after `workflow_id` field:
```rust
    pub instructions: Option<String>,
    pub ai_personality: Option<String>,
    pub user_role: Option<String>,
    pub start_date: Option<String>,
    pub target_end_date: Option<String>,
    pub settings: Option<String>,
```

- [ ] **Step 2: Add new fields to `SessionRow`**

In `crates/storage/src/rows/session.rs`, add after existing fields:
```rust
    pub project_id: Option<String>,
    pub conversation_type: Option<String>,
    pub pinned: bool,
```

Also add to `SessionListRow` struct:
```rust
    pub project_id: Option<String>,
    pub conversation_type: Option<String>,
    pub pinned: bool,
```

- [ ] **Step 3: Add `project_id` and `memory_type` to `SemanticFact`**

In `crates/cognitive/src/types.rs`, add to the SemanticFact struct:
```rust
    pub project_id: Option<String>,
    pub memory_type: String,
```

- [ ] **Step 4: Add `project_id` to `EpisodicMemory`**

```rust
    pub project_id: Option<String>,
```

- [ ] **Step 5: Add `project_id` to `ProceduralRule`**

```rust
    pub project_id: Option<String>,
```

- [ ] **Step 6: Fix all compilation errors from new fields**

Run: `cargo build --workspace 2>&1 | head -50`

New fields will cause errors wherever these structs are constructed. Fix each site:
- For `SemanticFact` construction: add `project_id: None, memory_type: "fact".to_string()`
- For `EpisodicMemory` construction: add `project_id: None`
- For `ProceduralRule` construction: add `project_id: None`
- For `ProjectRow` construction: add all 6 new fields as `None`
- For `SessionRow`/`SessionListRow`: add `project_id: None, conversation_type: None, pinned: false`

Use `cargo build --workspace 2>&1 | grep "error"` to find all sites. Fix them one by one.

- [ ] **Step 7: Run all tests**

Run: `cargo nextest run --workspace 2>&1 | tail -20`
Expected: All existing tests pass (new fields are optional/defaulted)

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat(storage): add project-scoped columns to ProjectRow, SessionRow, and cognitive types"
```

---

### Task 4: Update IPC Types — ProjectResponse, EntityKind, New Params

**Files:**
- Modify: `crates/desktop-shared/src/commands.rs:63-72` (ProjectResponse)
- Modify: `crates/desktop-shared/src/commands.rs:536-555` (Create/Update params)
- Modify: `crates/desktop-shared/src/types.rs:48-84` (EntityKind)
- Create: `crates/desktop-shared/src/entity_link_types.rs` (new IPC types)

- [ ] **Step 1: Add new fields to `ProjectResponse`**

In `crates/desktop-shared/src/commands.rs`, add to `ProjectResponse`:
```rust
    pub instructions: Option<serde_json::Value>,
    pub ai_personality: Option<String>,
    pub user_role: Option<String>,
    pub start_date: Option<String>,
    pub target_end_date: Option<String>,
    pub settings: Option<serde_json::Value>,
```

> **Note:** `description` already exists on `ProjectRow` and `ProjectResponse` — do not re-add it.

- [ ] **Step 2: Add new fields to `ProjectUpdateParams`**

Add to `ProjectUpdateParams`:
```rust
    pub instructions: Option<serde_json::Value>,
    pub ai_personality: Option<Option<String>>,
    pub user_role: Option<Option<String>>,
    pub start_date: Option<Option<String>>,
    pub target_end_date: Option<Option<String>>,
    pub settings: Option<serde_json::Value>,
```

- [ ] **Step 3: Add `Source` and `Conversation` to `EntityKind`**

In `crates/desktop-shared/src/types.rs`, add variants to the enum:
```rust
    Source,
    Conversation,
```

Update the `parse()` method to include:
```rust
    "source" => Some(EntityKind::Source),
    "conversation" => Some(EntityKind::Conversation),
```

- [ ] **Step 4: Create entity link IPC types**

Create `crates/desktop-shared/src/entity_link_types.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityLinkResponse {
    pub id: String,
    pub source_kind: String,
    pub source_id: String,
    pub target_kind: String,
    pub target_id: String,
    pub link_type: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityLinkCreateParams {
    pub source_kind: String,
    pub source_id: String,
    pub target_kind: String,
    pub target_id: String,
    pub link_type: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityLinksForEntityParams {
    pub kind: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedEntitiesResponse {
    pub tasks: Vec<ActionSummaryResponse>,
    pub notes: Vec<NoteSummaryResponse>,
    pub conversations: Vec<SessionSummaryResponse>,
    pub sources: Vec<ProjectSourceResponse>,
    pub objectives: Vec<ObjectiveSummaryResponse>,
    pub key_results: Vec<KeyResultSummaryResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionSummaryResponse {
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSummaryResponse {
    pub id: String,
    pub title: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummaryResponse {
    pub key: String,
    pub title: Option<String>,
    pub conversation_type: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectiveSummaryResponse {
    pub id: String,
    pub title: String,
    pub progress: f64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyResultSummaryResponse {
    pub id: String,
    pub title: String,
    pub progress: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSourceResponse {
    pub id: String,
    pub project_id: String,
    pub source_type: String,
    pub title: String,
    pub content: Option<String>,
    pub url: Option<String>,
    pub file_path: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSourceCreateParams {
    pub project_id: String,
    pub source_type: String,
    pub title: String,
    pub content: Option<String>,
    pub url: Option<String>,
    pub file_path: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSourceUpdateParams {
    pub id: String,
    pub title: Option<String>,
    pub content: Option<Option<String>>,
    pub url: Option<Option<String>>,
    pub metadata: Option<serde_json::Value>,
    pub tags: Option<Vec<String>>,
}
```

- [ ] **Step 5: Register new module in `desktop-shared/src/lib.rs`**

Add to `crates/desktop-shared/src/lib.rs`:
```rust
pub mod entity_link_types;
pub use entity_link_types::*;
```

- [ ] **Step 6: Fix compilation and verify**

Run: `cargo build -p desktop-shared 2>&1 | head -20`
Expected: Build success

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(desktop-shared): add EntityLink, ProjectSource IPC types and extend ProjectResponse"
```

---

## Chunk 2: Repositories (Phase 1b)

### Task 5: Implement EntityLinkRepo

**Files:**
- Create: `crates/storage/src/repos/entity_link_repo.rs`
- Test: inline `#[cfg(test)] mod tests`

- [ ] **Step 1: Write failing tests for EntityLinkRepo**

Create `crates/storage/src/repos/entity_link_repo.rs` with test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoragePool;

    async fn setup() -> EntityLinkRepo {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        EntityLinkRepo::new(pool.inner().clone())
    }

    #[tokio::test]
    async fn test_create_and_list() {
        let repo = setup().await;
        let link = repo.create("task", "task-1", "note", "note-1", "related", None).await.unwrap();
        assert_eq!(link.source_kind, "task");
        assert_eq!(link.link_type, "related");

        let links = repo.list_by_entity("task", "task-1").await.unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target_id, "note-1");
    }

    #[tokio::test]
    async fn test_delete() {
        let repo = setup().await;
        let link = repo.create("task", "t1", "note", "n1", "related", None).await.unwrap();
        repo.delete(&link.id).await.unwrap();
        let links = repo.list_by_entity("task", "t1").await.unwrap();
        assert!(links.is_empty());
    }

    #[tokio::test]
    async fn test_unique_constraint() {
        let repo = setup().await;
        repo.create("task", "t1", "note", "n1", "related", None).await.unwrap();
        // Same link again should fail (unique constraint violation)
        let result = repo.create("task", "t1", "note", "n1", "related", None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_bidirectional_query() {
        let repo = setup().await;
        repo.create("task", "t1", "note", "n1", "related", None).await.unwrap();
        // Should find from both directions
        let from_task = repo.list_by_entity("task", "t1").await.unwrap();
        let from_note = repo.list_by_entity("note", "n1").await.unwrap();
        assert_eq!(from_task.len(), 1);
        assert_eq!(from_note.len(), 1);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p storage -E 'test(entity_link)' 2>&1`
Expected: Compilation errors (struct/methods not defined yet)

- [ ] **Step 3: Implement EntityLinkRepo**

```rust
// crates/storage/src/repos/entity_link_repo.rs
use crate::rows::EntityLinkRow;
use crate::SqlitePool;
use common::Result;

#[derive(Clone)]
pub struct EntityLinkRepo {
    pool: SqlitePool,
}

impl EntityLinkRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        source_kind: &str,
        source_id: &str,
        target_kind: &str,
        target_id: &str,
        link_type: &str,
        metadata: Option<&str>,
    ) -> Result<EntityLinkRow> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query_as::<_, EntityLinkRow>(
            "INSERT INTO entity_links (id, source_kind, source_id, target_kind, target_id, link_type, metadata)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             RETURNING *"
        )
        .bind(&id)
        .bind(source_kind)
        .bind(source_id)
        .bind(target_kind)
        .bind(target_id)
        .bind(link_type)
        .bind(metadata)
        .fetch_one(self.pool.get())
        .await
        .map_err(Into::into)
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM entity_links WHERE id = ?")
            .bind(id)
            .execute(self.pool.get())
            .await?;
        Ok(())
    }

    pub async fn list_by_entity(&self, kind: &str, id: &str) -> Result<Vec<EntityLinkRow>> {
        sqlx::query_as::<_, EntityLinkRow>(
            "SELECT * FROM entity_links
             WHERE (source_kind = ? AND source_id = ?)
                OR (target_kind = ? AND target_id = ?)
             ORDER BY created_at DESC"
        )
        .bind(kind).bind(id)
        .bind(kind).bind(id)
        .fetch_all(self.pool.get())
        .await
        .map_err(Into::into)
    }

    /// Typed convenience: link task to note with "related" type
    pub async fn link_task_to_note(&self, task_id: &str, note_id: &str) -> Result<EntityLinkRow> {
        self.create("task", task_id, "note", note_id, "related", None).await
    }

    /// Typed convenience: link conversation (session key) to task
    pub async fn link_conversation_to_task(&self, session_key: &str, task_id: &str) -> Result<EntityLinkRow> {
        self.create("conversation", session_key, "task", task_id, "related", None).await
    }

    /// Typed convenience: link conversation to note
    pub async fn link_conversation_to_note(&self, session_key: &str, note_id: &str) -> Result<EntityLinkRow> {
        self.create("conversation", session_key, "note", note_id, "related", None).await
    }

    /// Get all links where a project is either source or target
    pub async fn get_project_links(&self, project_id: &str) -> Result<Vec<EntityLinkRow>> {
        sqlx::query_as::<_, EntityLinkRow>(
            "SELECT * FROM entity_links
             WHERE (source_kind = 'project' AND source_id = ?)
                OR (target_kind = 'project' AND target_id = ?)
             ORDER BY created_at DESC"
        )
        .bind(project_id).bind(project_id)
        .fetch_all(self.pool.get())
        .await
        .map_err(Into::into)
    }
}
```

Note: `get_linked_entities()` requires joining across multiple tables. Implement in AppCore handler layer where all repos are available — not in EntityLinkRepo itself (which only has SqlitePool).

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p storage -E 'test(entity_link)' 2>&1`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/storage/src/repos/entity_link_repo.rs
git commit -m "feat(storage): implement EntityLinkRepo with CRUD and typed convenience methods"
```

---

### Task 6: Implement ProjectSourceRepo

**Files:**
- Create: `crates/storage/src/repos/project_source_repo.rs`
- Test: inline `#[cfg(test)] mod tests`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoragePool;

    async fn setup() -> (ProjectSourceRepo, crate::repos::ProjectRepo) {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repos = crate::repos::Repos::from_pool(&pool);
        // Create an area and project first (neither AreaRow nor ProjectRow derives Default — use explicit fields)
        repos.areas.create(&crate::rows::AreaRow {
            id: "area-1".into(),
            name: "Work".into(),
            description: None,
            icon: None,
            color: "blue".into(),
            sort_order: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }).await.unwrap();
        repos.projects.create(&crate::rows::ProjectRow {
            id: "proj-1".into(),
            area_id: "area-1".into(),
            name: "Test".into(),
            description: None,
            status: "active".into(),
            priority: None,
            workflow_id: None,
            instructions: None,
            ai_personality: None,
            user_role: None,
            start_date: None,
            target_end_date: None,
            settings: None,
            color: None,
            icon: None,
            sort_order: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }).await.unwrap();
        (ProjectSourceRepo::new(pool.inner().clone()), repos.projects)
    }

    #[tokio::test]
    async fn test_create_and_list() {
        let (repo, _) = setup().await;
        let source = repo.create("proj-1", "link", "React Docs", Some("content"), Some("https://react.dev"), None).await.unwrap();
        assert_eq!(source.project_id, "proj-1");
        assert_eq!(source.source_type, "link");

        let sources = repo.list_by_project("proj-1").await.unwrap();
        assert_eq!(sources.len(), 1);
    }

    #[tokio::test]
    async fn test_delete() {
        let (repo, _) = setup().await;
        let source = repo.create("proj-1", "snippet", "Auth flow", Some("code"), None, None).await.unwrap();
        repo.delete(&source.id).await.unwrap();
        let sources = repo.list_by_project("proj-1").await.unwrap();
        assert!(sources.is_empty());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p storage -E 'test(project_source)' 2>&1`

- [ ] **Step 3: Implement ProjectSourceRepo**

```rust
// crates/storage/src/repos/project_source_repo.rs
use crate::rows::ProjectSourceRow;
use crate::SqlitePool;
use common::Result;

#[derive(Clone)]
pub struct ProjectSourceRepo {
    pool: SqlitePool,
}

impl ProjectSourceRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        project_id: &str,
        source_type: &str,
        title: &str,
        content: Option<&str>,
        url: Option<&str>,
        file_path: Option<&str>,
    ) -> Result<ProjectSourceRow> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query_as::<_, ProjectSourceRow>(
            "INSERT INTO project_sources (id, project_id, source_type, title, content, url, file_path)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             RETURNING *"
        )
        .bind(&id).bind(project_id).bind(source_type)
        .bind(title).bind(content).bind(url).bind(file_path)
        .fetch_one(self.pool.get())
        .await
        .map_err(Into::into)
    }

    pub async fn get(&self, id: &str) -> Result<Option<ProjectSourceRow>> {
        sqlx::query_as::<_, ProjectSourceRow>("SELECT * FROM project_sources WHERE id = ?")
            .bind(id)
            .fetch_optional(self.pool.get())
            .await
            .map_err(Into::into)
    }

    pub async fn list_by_project(&self, project_id: &str) -> Result<Vec<ProjectSourceRow>> {
        sqlx::query_as::<_, ProjectSourceRow>(
            "SELECT * FROM project_sources WHERE project_id = ? ORDER BY created_at DESC"
        )
        .bind(project_id)
        .fetch_all(self.pool.get())
        .await
        .map_err(Into::into)
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM project_sources WHERE id = ?")
            .bind(id)
            .execute(self.pool.get())
            .await?;
        Ok(())
    }

    pub async fn update_content(&self, id: &str, content: &str) -> Result<()> {
        sqlx::query(
            "UPDATE project_sources SET content = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?"
        )
        .bind(content).bind(id)
        .execute(self.pool.get())
        .await?;
        Ok(())
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p storage -E 'test(project_source)' 2>&1`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add crates/storage/src/repos/project_source_repo.rs
git commit -m "feat(storage): implement ProjectSourceRepo with CRUD"
```

---

### Task 7: Extend Existing Repos and Repos Aggregate

**Files:**
- Modify: `crates/storage/src/repos/project_repo.rs` (add project-scoped queries)
- Modify: `crates/storage/src/repos/mod.rs:67-123` (Repos struct + from_pool)
- Modify: `crates/cognitive/src/repos/semantic_fact_repo.rs` (add project queries)
- Modify: `crates/storage/src/repos/session.rs` (add project queries)

- [ ] **Step 1: Add project-scoped queries to `ProjectRepo`**

In `crates/storage/src/repos/project_repo.rs`, add methods:

```rust
    pub async fn update_instructions(&self, id: &str, instructions: &str) -> Result<()> {
        sqlx::query("UPDATE projects SET instructions = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?")
            .bind(instructions).bind(id)
            .execute(self.pool.get()).await?;
        Ok(())
    }

    pub async fn update_user_role(&self, id: &str, role: &str) -> Result<()> {
        sqlx::query("UPDATE projects SET user_role = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?")
            .bind(role).bind(id)
            .execute(self.pool.get()).await?;
        Ok(())
    }
```

- [ ] **Step 2: Add project-scoped queries to SemanticFactRepo**

> **Important:** In addition to the new query methods below, you must also update the existing `upsert`/`insert` methods in `SemanticFactRepo` to bind the new `project_id` column. Otherwise, inserted facts will always have `project_id = NULL`.

In `crates/cognitive/src/repos/semantic_fact_repo.rs`, add:

```rust
    pub async fn list_by_project(&self, project_id: &str) -> Result<Vec<SemanticFact>> {
        sqlx::query_as::<_, SemanticFact>(
            "SELECT * FROM semantic_facts WHERE project_id = ? AND superseded_at IS NULL ORDER BY recorded_at DESC"
        )
        .bind(project_id)
        .fetch_all(self.pool.get()).await.map_err(Into::into)
    }

    pub async fn list_by_project_and_type(&self, project_id: &str, memory_type: &str) -> Result<Vec<SemanticFact>> {
        sqlx::query_as::<_, SemanticFact>(
            "SELECT * FROM semantic_facts WHERE project_id = ? AND memory_type = ? AND superseded_at IS NULL ORDER BY recorded_at DESC"
        )
        .bind(project_id).bind(memory_type)
        .fetch_all(self.pool.get()).await.map_err(Into::into)
    }
```

- [ ] **Step 3: Add project-scoped queries to SessionRepo**

> **Note:** `SessionListRow` was updated in Task 3 Step 2 to include `project_id`, `conversation_type`, and `pinned` fields. The `SELECT s.*` in the query below will pick up these new columns.

In `crates/storage/src/repos/session.rs`, add:

```rust
    pub async fn list_by_project(&self, project_id: &str) -> Result<Vec<SessionListRow>> {
        sqlx::query_as::<_, SessionListRow>(
            "SELECT s.*, COUNT(m.id) as message_count
             FROM sessions s LEFT JOIN session_messages m ON m.session_key = s.key
             WHERE s.project_id = ?
             GROUP BY s.key
             ORDER BY s.updated_at DESC"
        )
        .bind(project_id)
        .fetch_all(self.pool.get()).await.map_err(Into::into)
    }
```

- [ ] **Step 4: Add new repos to `Repos` aggregate**

In `crates/storage/src/repos/mod.rs`, add to the struct:
```rust
    pub entity_links: EntityLinkRepo,
    pub project_sources: ProjectSourceRepo,
```

And in `from_pool()`:
```rust
    entity_links: EntityLinkRepo::new(pool.inner().clone()),
    project_sources: ProjectSourceRepo::new(pool.inner().clone()),
```

Add to `crates/storage/src/repos/mod.rs`:
```rust
pub mod entity_link_repo;
pub mod project_source_repo;
pub use entity_link_repo::EntityLinkRepo;
pub use project_source_repo::ProjectSourceRepo;
```

- [ ] **Step 5: Build and test**

Run: `cargo nextest run --workspace 2>&1 | tail -20`
Expected: All tests pass

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(storage): extend repos with project-scoped queries and add to Repos aggregate"
```

---

## Chunk 3: Handlers & Commands (Phase 1c)

> **Prerequisites:** Chunks 1-2 must be complete. Specifically:
> - `EntityKind::Source` and `EntityKind::Conversation` variants must exist (Task 4 Step 3, Chunk 1).
> - `entity_link_types` module must exist in `desktop-shared` (Task 4 Step 4, Chunk 1).
> - All new repos (`EntityLinkRepo`, `ProjectSourceRepo`) and repo extensions must be in place (Chunk 2).
> - New Tauri command files also need corresponding routes in `dev_server.rs` (the `dispatch_dev` pattern). Add a dev-server dispatch step for each new command file.

### Task 8: AppCore Handlers — Entity Links

**Files:**
- Create: `crates/app-core/src/handlers/entity_links.rs`
- Modify: `crates/app-core/src/handlers/mod.rs` (add module)

- [ ] **Step 1: Create entity link handlers**

```rust
// crates/app-core/src/handlers/entity_links.rs
use crate::state::{AppCore, HandlerResult};
use common::Result;
use desktop_shared::entity_link_types::*;
use storage::rows::EntityLinkRow;

fn row_to_response(row: EntityLinkRow) -> EntityLinkResponse {
    EntityLinkResponse {
        id: row.id,
        source_kind: row.source_kind,
        source_id: row.source_id,
        target_kind: row.target_kind,
        target_id: row.target_id,
        link_type: row.link_type,
        metadata: row.metadata.and_then(|m| serde_json::from_str(&m).ok()),
        created_at: row.created_at,
    }
}

impl AppCore {
    pub async fn entity_link_create(&self, params: EntityLinkCreateParams) -> HandlerResult<EntityLinkResponse> {
        let link_type = params.link_type.as_deref().unwrap_or("related");
        let metadata_str = params.metadata.as_ref().map(|m| m.to_string());
        let row = self.repos.entity_links.create(
            &params.source_kind, &params.source_id,
            &params.target_kind, &params.target_id,
            link_type,
            metadata_str.as_deref(),
        ).await?;

        let updates = vec![]; // Entity links don't emit specific entity updates
        Ok((row_to_response(row), updates))
    }

    pub async fn entity_link_delete(&self, id: String) -> HandlerResult<bool> {
        self.repos.entity_links.delete(&id).await?;
        Ok((true, vec![]))
    }

    pub async fn entity_links_for_entity(&self, kind: String, id: String) -> Result<LinkedEntitiesResponse> {
        let links = self.repos.entity_links.list_by_entity(&kind, &id).await?;

        let mut tasks = vec![];
        let mut notes = vec![];
        let mut conversations = vec![];
        let mut objectives = vec![];
        let mut key_results = vec![];
        let mut sources = vec![];

        for link in &links {
            // Determine the "other" entity (the one that isn't the queried entity)
            let (other_kind, other_id) = if link.source_kind == kind && link.source_id == id {
                (&link.target_kind, &link.target_id)
            } else {
                (&link.source_kind, &link.source_id)
            };

            match other_kind.as_str() {
                "task" => {
                    if let Ok(Some(action)) = self.repos.actions.get(other_id).await {
                        tasks.push(ActionSummaryResponse {
                            id: action.id, title: action.title, status: action.status,
                            priority: action.priority,
                        });
                    }
                }
                "note" => {
                    // NoteRepo is accessed via the feature_notes crate's repo.
                    // Implementation: query notes by IDs from links, map to NoteSummaryResponse.
                    // Example: if let Ok(Some(note)) = self.note_repo.get(other_id).await {
                    //     notes.push(NoteSummaryResponse { id: note.id, title: note.title, updated_at: note.updated_at.to_rfc3339() });
                    // }
                }
                "conversation" => {
                    if let Ok(Some(session)) = self.repos.sessions.get(other_id).await {
                        conversations.push(SessionSummaryResponse {
                            key: session.key,
                            title: session.metadata.as_object()
                                .and_then(|m| m.get("title"))
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            conversation_type: session.conversation_type.clone(),
                            updated_at: session.updated_at.to_rfc3339(),
                        });
                    }
                }
                "objective" => {
                    if let Ok(Some(obj)) = self.repos.objectives.get(other_id).await {
                        objectives.push(ObjectiveSummaryResponse {
                            id: obj.id, title: obj.title, progress: obj.progress,
                            status: obj.status,
                        });
                    }
                }
                "key_result" => {
                    if let Ok(Some(kr)) = self.repos.key_results.get(other_id).await {
                        key_results.push(KeyResultSummaryResponse {
                            id: kr.id, title: kr.title, progress: kr.progress,
                        });
                    }
                }
                "source" => {
                    if let Ok(Some(src)) = self.repos.project_sources.get(other_id).await {
                        sources.push(ProjectSourceResponse {
                            id: src.id, project_id: src.project_id,
                            source_type: src.source_type, title: src.title,
                            content: src.content, url: src.url, file_path: src.file_path,
                            metadata: src.metadata.and_then(|m| serde_json::from_str(&m).ok()),
                            tags: serde_json::from_str(&src.tags).unwrap_or_default(),
                            created_at: src.created_at, updated_at: src.updated_at,
                        });
                    }
                }
                _ => {}
            }
        }

        Ok(LinkedEntitiesResponse { tasks, notes, conversations, sources, objectives, key_results })
    }
}
```

Note: The `note` lookup branch depends on the NoteRepo API — fill in during implementation based on the actual note repo's `get()` method.

- [ ] **Step 2: Register module in handlers/mod.rs**

Add `pub mod entity_links;` to `crates/app-core/src/handlers/mod.rs`.

- [ ] **Step 3: Build and verify**

Run: `cargo build -p app-core 2>&1 | head -30`
Expected: Build success

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(app-core): add entity link handlers — create, delete, list linked entities"
```

---

### Task 9: AppCore Handlers — Project Sources

**Files:**
- Create: `crates/app-core/src/handlers/project_sources.rs`

- [ ] **Step 1: Implement project source handlers**

```rust
// crates/app-core/src/handlers/project_sources.rs
use crate::state::{AppCore, EntityUpdate, HandlerResult};
use common::Result;
use desktop_shared::entity_link_types::*;
use desktop_shared::types::EntityKind;

impl AppCore {
    pub async fn project_source_create(&self, params: ProjectSourceCreateParams) -> HandlerResult<ProjectSourceResponse> {
        let row = self.repos.project_sources.create(
            &params.project_id, &params.source_type, &params.title,
            params.content.as_deref(), params.url.as_deref(), params.file_path.as_deref(),
        ).await?;

        let response = ProjectSourceResponse {
            id: row.id.clone(), project_id: row.project_id,
            source_type: row.source_type, title: row.title,
            content: row.content, url: row.url, file_path: row.file_path,
            metadata: row.metadata.and_then(|m| serde_json::from_str(&m).ok()),
            tags: serde_json::from_str(&row.tags).unwrap_or_default(),
            created_at: row.created_at, updated_at: row.updated_at,
        };

        let updates = vec![EntityUpdate { kind: EntityKind::Source, id: response.id.clone() }];
        Ok((response, updates))
    }

    pub async fn project_source_delete(&self, id: String) -> HandlerResult<bool> {
        self.repos.project_sources.delete(&id).await?;
        Ok((true, vec![EntityUpdate { kind: EntityKind::Source, id }]))
    }

    pub async fn project_source_list(&self, project_id: String) -> Result<Vec<ProjectSourceResponse>> {
        let rows = self.repos.project_sources.list_by_project(&project_id).await?;
        Ok(rows.into_iter().map(|row| ProjectSourceResponse {
            id: row.id, project_id: row.project_id,
            source_type: row.source_type, title: row.title,
            content: row.content, url: row.url, file_path: row.file_path,
            metadata: row.metadata.and_then(|m| serde_json::from_str(&m).ok()),
            tags: serde_json::from_str(&row.tags).unwrap_or_default(),
            created_at: row.created_at, updated_at: row.updated_at,
        }).collect())
    }
}
```

- [ ] **Step 2: Register and build**

Add `pub mod project_sources;` to handlers/mod.rs.
Run: `cargo build -p app-core 2>&1 | head -20`

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(app-core): add project source handlers — create, delete, list"
```

---

### Task 10: Extend Project Handlers with New Fields

> **Prerequisite:** Task 7 (Chunk 2) must be complete — `update_instructions` and `update_user_role` methods on `ProjectRepo` are required by Step 3.

**Files:**
- Modify: `crates/app-core/src/handlers/projects.rs:9-175`

- [ ] **Step 1: Update `project_to_response` to include new fields**

In `crates/app-core/src/handlers/projects.rs`, update the `project_to_response` helper and `build_project_response` to include `instructions`, `ai_personality`, `user_role`, `start_date`, `target_end_date`, `settings`, `description` from the `ProjectRow`.

- [ ] **Step 2: Update `project_update` handler to handle new fields**

Add handling for the new `ProjectUpdateParams` fields in the update handler. The `ProjectPatch` struct in `project_repo.rs` also needs updating to include the new fields.

- [ ] **Step 3: Add new handlers for instructions and role**

```rust
impl AppCore {
    pub async fn project_update_instructions(&self, id: String, instructions: serde_json::Value) -> HandlerResult<ProjectResponse> {
        let json_str = serde_json::to_string(&instructions)?;
        self.repos.projects.update_instructions(&id, &json_str).await?;
        let response = self.build_project_response(&id).await?;
        let updates = vec![EntityUpdate { kind: EntityKind::Project, id }];
        Ok((response, updates))
    }

    pub async fn project_update_role(&self, id: String, role: String) -> HandlerResult<ProjectResponse> {
        self.repos.projects.update_user_role(&id, &role).await?;
        let response = self.build_project_response(&id).await?;
        let updates = vec![EntityUpdate { kind: EntityKind::Project, id }];
        Ok((response, updates))
    }
}
```

- [ ] **Step 4: Build and test**

Run: `cargo nextest run -p app-core 2>&1 | tail -20`

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(app-core): extend project handlers with instructions, role, and new response fields"
```

---

### Task 11: Tauri Commands — Entity Links, Project Sources, Project Extensions

**Files:**
- Create: `crates/desktop/src/commands/entity_links.rs`
- Create: `crates/desktop/src/commands/project_sources.rs`
- Modify: `crates/desktop/src/commands/projects.rs` (add new commands)
- Modify: `crates/desktop/src/commands/mod.rs` (register modules)
- Modify: `crates/desktop/tauri.conf.json` or command registration (register new commands)

- [ ] **Step 1: Create entity link Tauri commands**

```rust
// crates/desktop/src/commands/entity_links.rs
use app_core::AppCore;
use desktop_shared::entity_link_types::*;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn entity_link_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: EntityLinkCreateParams,
) -> Result<EntityLinkResponse, app_core::ApiError> {
    let (result, updates) = state.entity_link_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn entity_link_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, app_core::ApiError> {
    let (result, updates) = state.entity_link_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn entity_links_for_entity(
    state: State<'_, Arc<AppCore>>,
    kind: String,
    id: String,
) -> Result<LinkedEntitiesResponse, app_core::ApiError> {
    state.entity_links_for_entity(kind, id).await.map_err(Into::into)
}
```

- [ ] **Step 2: Create project source Tauri commands** (same pattern)

- [ ] **Step 3: Add new project commands** (`project_update_instructions`, `project_update_role`)

- [ ] **Step 4: Register all new commands in mod.rs and the Tauri builder**

Find the `.invoke_handler(tauri::generate_handler![...])` call and add all new commands.

- [ ] **Step 5: Add dev-server dispatch routes**

In `crates/desktop/src/dev_server.rs` (the `dispatch_dev` function), add matching routes for all new commands: `entity_link_create`, `entity_link_delete`, `entity_links_for_entity`, `project_source_create`, `project_source_delete`, `project_source_list`, `project_update_instructions`, `project_update_role`. Each route should delegate to the corresponding `AppCore` method identically to the Tauri command but discard entity updates.

- [ ] **Step 6: Build full desktop app**

Run: `cargo build -p desktop 2>&1 | tail -20`
Expected: Build success

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(desktop): add Tauri commands for entity links, project sources, and project extensions"
```

---

### Task 12: Frontend Types and Hooks

**Files:**
- Modify: `desktop-ui/src/shared/types/tasks.ts:47-56` (Project interface)
- Create: `desktop-ui/src/shared/types/entity-links.ts`
- Create: `desktop-ui/src/shared/hooks/useEntityLinks.ts`
- Create: `desktop-ui/src/shared/hooks/useProjectSources.ts`
- Create: `desktop-ui/src/shared/hooks/useProjectConversations.ts`
- Create: `desktop-ui/src/shared/hooks/useProjectMemories.ts`

- [ ] **Step 1: Update Project interface**

In `desktop-ui/src/shared/types/tasks.ts`, add to `Project`:
```typescript
  instructions?: {
    context?: string;
    guidelines?: string;
    constraints?: string;
    persona?: string;
  };
  aiPersonality?: string;
  userRole?: string;
  startDate?: string;
  targetEndDate?: string;
  settings?: Record<string, unknown>;
  description?: string;
```

Update `ProjectUpdateParams` similarly.

- [ ] **Step 2: Create entity link types**

```typescript
// desktop-ui/src/shared/types/entity-links.ts
export interface EntityLink {
  id: string;
  sourceKind: string;
  sourceId: string;
  targetKind: string;
  targetId: string;
  linkType: string;
  metadata?: Record<string, unknown>;
  createdAt: string;
}

export interface LinkedEntities {
  tasks: ActionSummary[];
  notes: NoteSummary[];
  conversations: SessionSummary[];
  sources: ProjectSource[];
  objectives: ObjectiveSummary[];
  keyResults: KeyResultSummary[];
}

export interface ActionSummary { id: string; title: string; status: string; priority?: string; }
export interface NoteSummary { id: string; title: string; updatedAt: string; }
export interface SessionSummary { key: string; title?: string; conversationType?: string; updatedAt: string; }
export interface ObjectiveSummary { id: string; title: string; progress: number; status: string; }
export interface KeyResultSummary { id: string; title: string; progress: number; }

export interface ProjectSource {
  id: string;
  projectId: string;
  sourceType: string;
  title: string;
  content?: string;
  url?: string;
  filePath?: string;
  metadata?: Record<string, unknown>;
  tags: string[];
  createdAt: string;
  updatedAt: string;
}
```

- [ ] **Step 3: Create hooks**

Create `useEntityLinks.ts`:
```typescript
import { useQuery } from './useQuery';
import type { LinkedEntities } from '../types/entity-links';

export function useEntityLinks(kind: string, id: string) {
  return useQuery<LinkedEntities>('entity_links_for_entity', { kind, id });
}
```

Create `useProjectSources.ts`, `useProjectConversations.ts`, `useProjectMemories.ts` following the same `useQuery`/`useMutation` patterns used throughout the codebase.

- [ ] **Step 4: Export from shared/hooks/index.ts and shared/types barrel**

- [ ] **Step 5: Verify lint**

Run: `cd desktop-ui && bun run lint:fix`

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(desktop-ui): add entity link types, project source types, and data hooks"
```

---

## Chunk 4: Context Engine (Phase 2)

> **Prerequisites:** All of Chunks 1-2 must be complete (schema changes, row types, repos). Specifically, Task 7 (Chunk 2) adds `list_by_project` and `list_by_project_and_type` to `SemanticFactRepo`, which are required by Task 14.

### Task 13: Extend SourceContext with project_id

**Files:**
- Modify: `crates/context_engine/src/source.rs:12-22` (SourceContext struct)
- Modify: all callers that construct SourceContext (search for `SourceContext {`)

- [ ] **Step 1: Add `project_id` field to SourceContext**

In `crates/context_engine/src/source.rs`, add:
```rust
    pub project_id: Option<String>,
```

- [ ] **Step 2: Fix all construction sites**

Run `cargo build --workspace 2>&1 | grep "error"` — find every site that constructs `SourceContext` and add `project_id: None` (or the actual project_id if available from the session).

- [ ] **Step 3: Build and test**

Run: `cargo nextest run --workspace 2>&1 | tail -20`

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(context-engine): add project_id field to SourceContext"
```

---

### Task 14: Implement ProjectContextSource

**Files:**
- Create: `crates/agent/src/context_sources/project.rs`
- Modify: `crates/agent/src/context_sources/mod.rs` (register)

- [ ] **Step 1: Create ProjectContextSource**

```rust
// crates/agent/src/context_sources/project.rs
use async_trait::async_trait;
use cognitive::repos::SemanticFactRepo;
use context_engine::source::{ContextSource, SourceContext};
use storage::repos::Repos;

pub struct ProjectContextSource {
    repos: Repos,
    semantic_repo: SemanticFactRepo,
}

impl ProjectContextSource {
    pub fn new(repos: Repos, semantic_repo: SemanticFactRepo) -> Self {
        Self { repos, semantic_repo }
    }
}

#[async_trait]
impl ContextSource for ProjectContextSource {
    fn name(&self) -> &str { "project" }
    fn priority(&self) -> u8 { 80 }

    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        let project_id = ctx.project_id.as_deref()?;
        let project = self.repos.projects.get(project_id).await.ok()??;

        let mut sections = Vec::new();

        // 1. Instructions
        if let Some(instructions_json) = &project.instructions {
            if let Ok(instructions) = serde_json::from_str::<serde_json::Value>(instructions_json) {
                if let Some(context) = instructions.get("context").and_then(|v| v.as_str()) {
                    if !context.is_empty() {
                        sections.push(format!("## Project Context\n{}", context));
                    }
                }
                if let Some(guidelines) = instructions.get("guidelines").and_then(|v| v.as_str()) {
                    if !guidelines.is_empty() {
                        sections.push(format!("## Guidelines\n{}", guidelines));
                    }
                }
                if let Some(constraints) = instructions.get("constraints").and_then(|v| v.as_str()) {
                    if !constraints.is_empty() {
                        sections.push(format!("## Constraints\n{}", constraints));
                    }
                }
            }
        }

        // 2. User role
        if let Some(role) = &project.user_role {
            if !role.is_empty() {
                sections.push(format!("## User's Role\n{}", role));
            }
        }

        // 3. AI personality
        if let Some(personality) = &project.ai_personality {
            if !personality.is_empty() {
                sections.push(format!("## Communication Style\n{}", personality));
            }
        }

        // 4. Project-scoped memories (decisions, milestones)
        if let Ok(facts) = self.semantic_repo.list_by_project(project_id).await {
            if !facts.is_empty() {
                let memory_text: Vec<String> = facts.iter().take(10).map(|f| {
                    format!("- [{}] {} {} {}", f.memory_type, f.subject, f.predicate, f.object)
                }).collect();
                sections.push(format!("## Project Memories\n{}", memory_text.join("\n")));
            }
        }

        if sections.is_empty() {
            None
        } else {
            Some(format!("# Project: {}\n\n{}", project.name, sections.join("\n\n")))
        }
    }
}
```

Note: RAG over project sources (SourceRetriever) is deferred — this initial implementation provides static context only.

> **TODO (deferred):** Add a follow-up Task 15.5 to implement RAG/SourceRetriever integration that retrieves relevant project source content via vector search (LanceDB embeddings). This should query `project_sources` by `embedding_id`, rank by relevance to the current conversation, and append the top-k results to the context. This is NOT required for the initial implementation.

- [ ] **Step 2: Register in context sources mod.rs**

Add to the context source list in `crates/agent/src/agent_loop/builder.rs` where other context sources (AreaSource, etc.) are registered. Also add `pub mod project;` to `crates/agent/src/context_sources/mod.rs`.

- [ ] **Step 3: Build and test**

Run: `cargo build -p agent 2>&1 | head -20`

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(agent): implement ProjectContextSource — injects project instructions, role, memories into AI context"
```

---

### Task 15: Project Memory Handlers

**Files:**
- Create: `crates/app-core/src/handlers/project_memories.rs`
- Create: `crates/desktop/src/commands/project_memories.rs`

- [ ] **Step 1: Create memory handlers**

```rust
// crates/app-core/src/handlers/project_memories.rs
use crate::state::AppCore;
use common::Result;
use cognitive::types::SemanticFact;

impl AppCore {
    pub async fn project_memories_list(&self, project_id: String) -> Result<Vec<SemanticFact>> {
        // Note: SemanticFactRepo is NOT in `Repos` (it's in the cognitive crate).
        // AppCore must hold a SemanticFactRepo directly, constructed as:
        //   SemanticFactRepo::new(cognitive_pool)
        // Access via self.semantic_fact_repo (add field to AppCore if not present).
        self.semantic_fact_repo.list_by_project(&project_id).await
    }

    pub async fn project_memories_by_type(&self, project_id: String, memory_type: String) -> Result<Vec<SemanticFact>> {
        self.semantic_fact_repo.list_by_project_and_type(&project_id, &memory_type).await
    }
}
```

- [ ] **Step 2: Create Tauri commands** (same thin-wrapper pattern)

- [ ] **Step 3: Register and build**

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(app-core): add project memory handlers — list by project, filter by type"
```

---

## Chunk 5: Enhanced Project Detail UI (Phase 3)

> **Prerequisites:** Chunks 1-3 must be complete. Phase 2 IPC commands (`project_update_instructions`, `project_update_role`, `project_context_preview`) from Chunk 3 Task 10 must exist. All frontend hooks from Chunk 3 Task 12 must be in place.

### Task 16: Rewrite ProjectDetailPage — 3-Panel Layout Shell

**Files:**
- Modify: `desktop-ui/src/features/tasks/pages/ProjectDetailPage.tsx` (full rewrite, 499 lines)
- Create: `desktop-ui/src/features/tasks/components/project-detail/ProjectDetailHeader.tsx`
- Create: `desktop-ui/src/features/tasks/components/project-detail/ProjectLeftPanel.tsx`
- Create: `desktop-ui/src/features/tasks/components/project-detail/ProjectEntityPanel.tsx`
- Create: `desktop-ui/src/features/tasks/components/project-detail/ProjectChatInput.tsx`

- [ ] **Step 1: Create ProjectDetailHeader component**

> **Note:** Named `ProjectDetailHeader` to avoid collision with the existing `ProjectHeader.tsx` component.

Header bar with: back button, color dot, project name (inline editable), status badge, Day/Week/Month toggle, area + role display.

Follow existing pattern from current ProjectDetailPage header section.

- [ ] **Step 2: Create ProjectLeftPanel component**

Grok-style left panel with:
- Instructions card (clickable → opens editor)
- Sources card (clickable → opens manager)
- My Role card (clickable → opens editor)
- Separator line
- Conversations list (fetched via `useProjectConversations`)
- "+ New conversation" button

- [ ] **Step 3: Create ProjectEntityPanel component**

Right panel with expandable/collapsible sections. Each section uses a shared `CollapsibleSection` component.

**Step 3a: Create shared CollapsibleSection component**

Create `desktop-ui/src/shared/components/CollapsibleSection.tsx`:

```typescript
import { useState, type ReactNode } from 'react';

interface CollapsibleSectionProps {
  title: string;
  icon?: ReactNode;
  count?: number | null;
  defaultOpen?: boolean;
  children: ReactNode;
}

export function CollapsibleSection({ title, icon, count, defaultOpen = false, children }: CollapsibleSectionProps) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <div>
      <button onClick={() => setOpen(!open)} className="flex items-center gap-2 w-full px-3 py-2 text-sm font-medium text-heading hover:bg-surface-hover rounded-md">
        <span>{open ? '\u25BE' : '\u25B8'}</span>
        {icon && <span>{icon}</span>}
        <span>{title}</span>
        {count != null && <span className="ml-auto text-xs text-muted">{count}</span>}
      </button>
      {open && <div className="pl-6 pr-3 pb-2">{children}</div>}
    </div>
  );
}
```

Export from `desktop-ui/src/shared/components/index.ts`.

**Step 3b: Use CollapsibleSection in ProjectEntityPanel**

Sections: OKRs, Tasks, Notes, Memories, Sources, Productivity.

- [ ] **Step 4: Create ProjectChatInput component**

Bottom persistent chat bar. Adapts existing `ChatInput` patterns from `features/chat/`.

- [ ] **Step 5: Rewrite ProjectDetailPage as 3-panel layout**

```typescript
export function ProjectDetailPage() {
  const { id } = useParams<{ id: string }>();
  // Note: Ensure 'project_get' IPC binding is registered on the frontend.
  // Alternatively, create a useProject(id) hook that calls the project_get Tauri command.
  const { data: project } = useQuery<Project>('project_get', { id });
  // ... other queries

  return (
    <div className="flex flex-col h-full">
      <ProjectDetailHeader project={project} />
      <div className="flex flex-1 overflow-hidden">
        <ProjectLeftPanel projectId={id} />
        <div className="flex-1 overflow-auto">
          {/* Timeline — placeholder for Task 17 */}
          <div className="p-6 text-muted">Timeline coming soon...</div>
        </div>
        <ProjectEntityPanel projectId={id} />
      </div>
      <ProjectChatInput projectId={id} />
    </div>
  );
}
```

- [ ] **Step 6: Lint and verify**

Run: `cd desktop-ui && bun run lint:fix && bun run build`

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(desktop-ui): rewrite ProjectDetailPage as 3-panel layout with left panel, entity panel, chat input"
```

---

### Task 17: Timeline Components

**Files:**
- Create: `desktop-ui/src/features/tasks/components/project-detail/ProjectTimeline.tsx`
- Create: `desktop-ui/src/features/tasks/components/project-detail/timeline/ActivityColumn.tsx`
- Create: `desktop-ui/src/features/tasks/components/project-detail/timeline/TasksColumn.tsx`
- Create: `desktop-ui/src/features/tasks/components/project-detail/timeline/NotesColumn.tsx`
- Create: `desktop-ui/src/features/tasks/components/project-detail/timeline/MemoriesColumn.tsx`

- [ ] **Step 1: Create ProjectTimeline shell**

Multi-column time grid. Adapts patterns from existing `DayColumnsView` in `features/dashboard/` (the actual component with the time-grid rendering).

Grid layout: `grid-template-columns: 50px 1fr 1fr 1fr 1fr` — time labels, Activity, Tasks, Notes, Memories.

Hourly rows with items positioned by their timestamp.

- [ ] **Step 2: Create ActivityColumn**

Shows work context blocks from the work-context feature. Uses existing work context data hooks.

> **Note:** `useContextTimeline` needs to be extended with an optional `projectId` parameter to filter activity data to the current project scope.

- [ ] **Step 3: Create TasksColumn**

Shows project tasks positioned on timeline by `created_at` or `updated_at`. Compact card format with status checkbox, title, priority dot.

- [ ] **Step 4: Create NotesColumn and MemoriesColumn**

Similar card-on-timeline pattern.

- [ ] **Step 5: Integrate into ProjectDetailPage**

Replace timeline placeholder with `<ProjectTimeline projectId={id} />`.

- [ ] **Step 6: Lint and build**

Run: `cd desktop-ui && bun run lint:fix && bun run build`

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(desktop-ui): add project timeline with Activity, Tasks, Notes, Memories columns"
```

---

### Task 18: Left Panel — Instructions, Sources, Role Editors

**Files:**
- Create: `desktop-ui/src/features/tasks/components/project-detail/panels/InstructionsPanel.tsx`
- Create: `desktop-ui/src/features/tasks/components/project-detail/panels/SourcesPanel.tsx`
- Create: `desktop-ui/src/features/tasks/components/project-detail/panels/RolePanel.tsx`
- Create: `desktop-ui/src/features/tasks/components/project-detail/panels/ContextPreview.tsx`

- [ ] **Step 1: Create InstructionsPanel**

Slide panel or modal with collapsible sections (Context, Guidelines, Constraints, Persona). Each section is a Tiptap-style text area. Saves via `project_update_instructions` IPC call.

Include "Preview AI Context" button that calls `project_context_preview` and shows the assembled text.

- [ ] **Step 2: Create SourcesPanel**

Two sections:
- "From Project Notes (auto-included)" — read-only list of notes linked to this project
- "External Sources" — add link, upload file, paste snippet. Delete button per source.

- [ ] **Step 3: Create RolePanel**

Simple textarea with save button. Calls `project_update_role`.

- [ ] **Step 4: Create ContextPreview**

Modal showing the assembled AI context as read-only formatted text.

- [ ] **Step 5: Wire panels to left panel cards**

In `ProjectLeftPanel`, clicking each card opens the corresponding panel (using `useState` or `SlidePanel` composite).

- [ ] **Step 6: Lint, build, and commit**

```bash
git add -A
git commit -m "feat(desktop-ui): add Instructions, Sources, Role editor panels with AI context preview"
```

---

### Task 19: Entity Sections for Right Panel

**Files:**
- Create: `desktop-ui/src/features/tasks/components/project-detail/entity-sections/OkrSection.tsx`
- Create: `desktop-ui/src/features/tasks/components/project-detail/entity-sections/TaskSection.tsx`
- Create: `desktop-ui/src/features/tasks/components/project-detail/entity-sections/NoteSection.tsx`
- Create: `desktop-ui/src/features/tasks/components/project-detail/entity-sections/MemorySection.tsx`
- Create: `desktop-ui/src/features/tasks/components/project-detail/entity-sections/SourceSection.tsx`
- Create: `desktop-ui/src/features/tasks/components/project-detail/entity-sections/ProductivitySection.tsx`

- [ ] **Step 1: Create each section component**

Each follows the `CollapsibleSection` pattern from Task 16, with section-specific content:
- **OkrSection**: Progress bars per objective, linked task counts
- **TaskSection**: Task list with checkbox, priority dot, status, link indicator
- **NoteSection**: Note titles, timestamps, link counts
- **MemorySection**: Type filter tabs (All, Decisions, Insights, Milestones, Patterns), memory cards with colored type badges
- **SourceSection**: External source list (links, files, snippets)
- **ProductivitySection**: Stats grid (velocity, focus score, overdue, on-track %) + AI insights text

- [ ] **Step 2: Wire into ProjectEntityPanel**

```typescript
<OkrSection projectId={projectId} defaultOpen />
<TaskSection projectId={projectId} defaultOpen />
<NoteSection projectId={projectId} defaultOpen />
<MemorySection projectId={projectId} />
<SourceSection projectId={projectId} />
<ProductivitySection projectId={projectId} />
```

- [ ] **Step 3: Lint, build, and commit**

```bash
git add -A
git commit -m "feat(desktop-ui): add entity sections for right panel — OKRs, Tasks, Notes, Memories, Sources, Productivity"
```

---

## Chunk 6: Role & Intelligence (Phase 4)

### Task 20: Role-Aware Context Injection

**Files:**
- Modify: `crates/agent/src/context_sources/project.rs` (enhance with role-based adjustments)

- [ ] **Step 1: Enhance ProjectContextSource with role-based prompt adjustments**

In `crates/agent/src/context_sources/project.rs`, update the `provide()` method to append role-based guidance. Add this after the existing user role section:

```rust
    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        let project_id = ctx.project_id.as_deref()?;
        let project = self.repos.projects.get(project_id).await.ok()??;

        let mut sections = Vec::new();

        // ... existing instructions, user_role, ai_personality sections ...

        // Role-aware prompt adjustment (enhanced)
        if let Some(role) = &project.user_role {
            if !role.is_empty() {
                sections.push(format!(
                    "## Role-Aware Guidance\n\
                     Based on the user's role as \"{}\", adjust your suggestions and coaching \
                     to focus on their responsibilities. Communicate in a way that's relevant \
                     to their position. Prioritize information and tasks that align with this role.",
                    role
                ));
            }
        }

        // ... existing memory section, return logic ...
    }
```

- [ ] **Step 2: Build and test**

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(agent): enhance ProjectContextSource with role-aware context injection"
```

---

### Task 21: Memory Type Detection in Cognitive Pipeline

**Files:**
- Modify: `crates/cognitive/src/extraction.rs` (or equivalent extraction module)

> **Location:** The memory type detection belongs in the **post-processing step** after LLM extraction, NOT in the `ExtractionHandler` trait itself. Specifically, add a `classify_memory_type(fact: &str) -> &str` function in the extraction module that runs after the LLM returns extracted facts but before they are persisted. This function inspects the extracted fact text and assigns the `memory_type` field.

- [ ] **Step 1: Add decision detection patterns**

When extracting facts from conversations, detect phrases:
- "decided to", "let's go with", "we'll use", "agreed on" → set `memory_type: "decision"`
- "completed", "shipped", "released", "launched", "finished" → set `memory_type: "milestone"`
- "noticed that", "pattern", "tends to", "usually" → set `memory_type: "pattern"`
- "realized", "learned", "discovered" → set `memory_type: "insight"`

- [ ] **Step 2: Propagate `project_id` from session to extracted facts**

When the extraction pipeline processes a conversation with `project_id`, set it on all extracted `SemanticFact` entries.

- [ ] **Step 3: Add inline tests for memory type detection**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_decision() {
        assert_eq!(classify_memory_type("decided to use PostgreSQL"), "decision");
        assert_eq!(classify_memory_type("let's go with React"), "decision");
        assert_eq!(classify_memory_type("agreed on the API design"), "decision");
    }

    #[test]
    fn test_classify_milestone() {
        assert_eq!(classify_memory_type("completed the auth module"), "milestone");
        assert_eq!(classify_memory_type("shipped v2.0"), "milestone");
        assert_eq!(classify_memory_type("launched the beta"), "milestone");
    }

    #[test]
    fn test_classify_pattern() {
        assert_eq!(classify_memory_type("noticed that builds are slower"), "pattern");
        assert_eq!(classify_memory_type("tends to break on Mondays"), "pattern");
    }

    #[test]
    fn test_classify_insight() {
        assert_eq!(classify_memory_type("realized the bottleneck is I/O"), "insight");
        assert_eq!(classify_memory_type("learned that caching helps"), "insight");
    }

    #[test]
    fn test_classify_default() {
        assert_eq!(classify_memory_type("the sky is blue"), "fact");
    }
}
```

- [ ] **Step 4: Build and test**

Run: `cargo nextest run -p cognitive -E 'test(classify)' 2>&1`

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(cognitive): add decision/milestone/pattern detection and project_id propagation in extraction pipeline"
```

---

### Task 22: Project Conversation Handlers

**Files:**
- Create: `crates/app-core/src/handlers/project_conversations.rs`
- Create: `crates/desktop/src/commands/project_conversations.rs`

- [ ] **Step 1: Implement project conversation handlers**

```rust
impl AppCore {
    pub async fn project_conversations_list(&self, project_id: String) -> Result<Vec<SessionSummaryResponse>> {
        let rows = self.repos.sessions.list_by_project(&project_id).await?;
        Ok(rows.into_iter().map(|r| SessionSummaryResponse {
            key: r.key,
            title: r.metadata.as_object()
                .and_then(|m| m.get("title"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            conversation_type: r.conversation_type.clone(),
            updated_at: r.updated_at.to_rfc3339(),
        }).collect())
    }

    pub async fn project_conversation_create(
        &self,
        project_id: String,
        conversation_type: Option<String>,
    ) -> HandlerResult<SessionSummaryResponse> {
        // TODO: Create a new session with project_id set, emit SessionCreated update.
        // Use existing session creation logic from session handler as a reference.
        // Steps:
        // 1. Generate a new session key
        // 2. Create SessionRow with project_id and conversation_type set
        // 3. Insert via self.repos.sessions.create(...)
        // 4. Return SessionSummaryResponse + entity updates
        todo!("Implement project conversation creation using session handler patterns")
    }
}
```

- [ ] **Step 2: Add inline tests for project_conversations_list**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require an AppCore instance with in-memory storage.
    // Follow the same test setup pattern used in other handler test modules.

    #[tokio::test]
    async fn test_list_empty_project_conversations() {
        // Setup AppCore with in-memory pool
        // Create a project
        // List conversations for the project — should return empty vec
    }

    #[tokio::test]
    async fn test_list_project_conversations_returns_matching() {
        // Setup AppCore with in-memory pool
        // Create a project and sessions with project_id set
        // List conversations — should return matching sessions
        // Verify title extraction from metadata works correctly
    }
}
```

- [ ] **Step 3: Create Tauri commands and register**

- [ ] **Step 4: Build and test**

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(app-core): add project conversation handlers — list and create with project scope"
```

---

### Task 23: Final Integration — Verify Full Flow

- [ ] **Step 1: Run full test suite**

Run: `cargo nextest run --workspace 2>&1 | tail -30`
Expected: All tests pass

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | tail -20`
Expected: Zero warnings

- [ ] **Step 3: Run frontend build**

Run: `cd desktop-ui && bun run lint:fix && bun run build`
Expected: Clean build

- [ ] **Step 4: Manual smoke test**

Run: `cargo tauri dev`
- Navigate to a project
- Verify 3-panel layout renders
- Verify left panel shows instructions/sources/role cards
- Verify right panel shows expandable entity sections
- Verify timeline renders (even if empty)

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "chore: final integration cleanup for project-centric AI workspace"
```
