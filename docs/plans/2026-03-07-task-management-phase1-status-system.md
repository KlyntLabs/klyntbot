# Phase 1: Status System Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace hardcoded todo/doing/done statuses with a flexible, per-project workflow system with custom status labels, colors, and status groups.

**Architecture:** New `status_workflows` and `status_labels` tables in SQLite. A `StatusWorkflowRepo` provides CRUD. Projects get an optional `workflow_id` FK. The `actions` table gains a `status_label_id` FK alongside the existing `status` TEXT field for backward compatibility during migration. Frontend renders dynamic status labels with colors.

**Tech Stack:** Rust (sqlx, serde), TypeScript (React), Tailwind v4 CSS tokens

---

## Task 1: Database Migration

**Files:**
- Create: `crates/storage/migrations/004_status_workflows.sql`

**Step 1: Write the migration SQL**

```sql
-- Status workflow system: custom status labels per project
PRAGMA foreign_keys = ON;

-- ============================================================
-- Status Workflows (named collections of status labels)
-- ============================================================
CREATE TABLE status_workflows (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    is_template     INTEGER NOT NULL DEFAULT 0,
    is_global_default INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- ============================================================
-- Status Labels (belong to a workflow)
-- ============================================================
CREATE TABLE status_labels (
    id              TEXT PRIMARY KEY,
    workflow_id     TEXT NOT NULL REFERENCES status_workflows(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    color           TEXT NOT NULL DEFAULT '#6b7280',
    status_group    TEXT NOT NULL CHECK(status_group IN ('not_started', 'active', 'done', 'stuck')),
    position        INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX idx_status_labels_workflow_id ON status_labels(workflow_id);

-- ============================================================
-- Link projects to workflows (NULL = use global default)
-- ============================================================
ALTER TABLE projects ADD COLUMN workflow_id TEXT REFERENCES status_workflows(id) ON DELETE SET NULL;

-- ============================================================
-- Link actions to status labels (nullable during migration)
-- ============================================================
ALTER TABLE actions ADD COLUMN status_label_id TEXT REFERENCES status_labels(id) ON DELETE SET NULL;
ALTER TABLE actions ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
CREATE INDEX idx_actions_status_label_id ON actions(status_label_id);
CREATE INDEX idx_actions_position ON actions(position);

-- ============================================================
-- Seed global default workflow
-- ============================================================
INSERT INTO status_workflows (id, name, is_template, is_global_default)
VALUES ('wf_default', 'Default', 0, 1);

INSERT INTO status_labels (id, workflow_id, name, color, status_group, position) VALUES
    ('sl_backlog',     'wf_default', 'Backlog',     '#6b7280', 'not_started', 0),
    ('sl_todo',        'wf_default', 'Todo',        '#3b82f6', 'not_started', 1),
    ('sl_in_progress', 'wf_default', 'In Progress', '#eab308', 'active',      2),
    ('sl_in_review',   'wf_default', 'In Review',   '#f97316', 'active',      3),
    ('sl_done',        'wf_default', 'Done',        '#22c55e', 'done',        4),
    ('sl_blocked',     'wf_default', 'Blocked',     '#ef4444', 'stuck',       5);

-- ============================================================
-- Seed template workflows
-- ============================================================
INSERT INTO status_workflows (id, name, is_template) VALUES
    ('wf_simple',  'Simple',          1),
    ('wf_swdev',   'Software Dev',    1),
    ('wf_content', 'Content Creation', 1);

-- Simple: Todo, In Progress, Done
INSERT INTO status_labels (id, workflow_id, name, color, status_group, position) VALUES
    ('sl_s_todo',     'wf_simple', 'Todo',        '#3b82f6', 'not_started', 0),
    ('sl_s_progress', 'wf_simple', 'In Progress', '#eab308', 'active',      1),
    ('sl_s_done',     'wf_simple', 'Done',        '#22c55e', 'done',        2);

-- Software Dev: Backlog, Todo, In Progress, In Review, Done, Blocked
INSERT INTO status_labels (id, workflow_id, name, color, status_group, position) VALUES
    ('sl_sw_backlog',  'wf_swdev', 'Backlog',     '#6b7280', 'not_started', 0),
    ('sl_sw_todo',     'wf_swdev', 'Todo',        '#3b82f6', 'not_started', 1),
    ('sl_sw_progress', 'wf_swdev', 'In Progress', '#eab308', 'active',      2),
    ('sl_sw_review',   'wf_swdev', 'In Review',   '#f97316', 'active',      3),
    ('sl_sw_done',     'wf_swdev', 'Done',        '#22c55e', 'done',        4),
    ('sl_sw_blocked',  'wf_swdev', 'Blocked',     '#ef4444', 'stuck',       5);

-- Content Creation: Idea, Drafting, Editing, Published
INSERT INTO status_labels (id, workflow_id, name, color, status_group, position) VALUES
    ('sl_cc_idea',    'wf_content', 'Idea',      '#a855f7', 'not_started', 0),
    ('sl_cc_draft',   'wf_content', 'Drafting',  '#3b82f6', 'active',      1),
    ('sl_cc_edit',    'wf_content', 'Editing',   '#f97316', 'active',      2),
    ('sl_cc_publish', 'wf_content', 'Published', '#22c55e', 'done',        3);

-- ============================================================
-- Migrate existing actions: map status text → status_label_id
-- ============================================================
UPDATE actions SET status_label_id = 'sl_todo'        WHERE status = 'todo'   AND status_label_id IS NULL;
UPDATE actions SET status_label_id = 'sl_in_progress' WHERE status = 'doing'  AND status_label_id IS NULL;
UPDATE actions SET status_label_id = 'sl_done'        WHERE status = 'done'   AND status_label_id IS NULL;
UPDATE actions SET status_label_id = 'sl_backlog'     WHERE status_label_id IS NULL;
```

**Step 2: Verify migration applies cleanly**

Run: `cargo build -p storage 2>&1 | head -50`
Expected: Compiles (sqlx checks migration at compile time)

**Step 3: Commit**

```bash
git add crates/storage/migrations/004_status_workflows.sql
git commit -m "feat(storage): add status_workflows and status_labels migration"
```

---

## Task 2: Row Structs

**Files:**
- Create: `crates/storage/src/rows/status.rs`
- Modify: `crates/storage/src/rows/mod.rs`
- Modify: `crates/storage/src/rows/action.rs`

**Step 1: Create the status row structs**

Create `crates/storage/src/rows/status.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusWorkflowRow {
    pub id: String,
    pub name: String,
    pub is_template: bool,
    pub is_global_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusLabelRow {
    pub id: String,
    pub workflow_id: String,
    pub name: String,
    pub color: String,
    pub status_group: String,
    pub position: i32,
    pub created_at: DateTime<Utc>,
}
```

**Step 2: Register the module in rows/mod.rs**

Add to `crates/storage/src/rows/mod.rs`:

```rust
pub mod status;
pub use status::{StatusLabelRow, StatusWorkflowRow};
```

**Step 3: Add status_label_id and position to ActionRow**

In `crates/storage/src/rows/action.rs`, add after the `status` field:

```rust
pub status_label_id: Option<String>,
pub position: i32,
```

**Step 4: Build and verify**

Run: `cargo build -p storage 2>&1 | head -50`
Expected: Compiles successfully

**Step 5: Commit**

```bash
git add crates/storage/src/rows/status.rs crates/storage/src/rows/mod.rs crates/storage/src/rows/action.rs
git commit -m "feat(storage): add StatusWorkflowRow, StatusLabelRow, update ActionRow"
```

---

## Task 3: StatusWorkflowRepo

**Files:**
- Create: `crates/storage/src/repos/status_workflow.rs`
- Modify: `crates/storage/src/repos/mod.rs`

**Step 1: Write the failing test**

At the bottom of `crates/storage/src/repos/status_workflow.rs`, add a test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoragePool;

    async fn setup() -> StatusWorkflowRepo {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StatusWorkflowRepo::new(pool.inner().clone())
    }

    #[tokio::test]
    async fn test_get_global_default() {
        let repo = setup().await;
        let wf = repo.get_global_default().await.unwrap();
        assert!(wf.is_some());
        let wf = wf.unwrap();
        assert_eq!(wf.id, "wf_default");
        assert!(wf.is_global_default);
    }

    #[tokio::test]
    async fn test_list_templates() {
        let repo = setup().await;
        let templates = repo.list_templates().await.unwrap();
        assert_eq!(templates.len(), 3);
    }

    #[tokio::test]
    async fn test_get_labels_for_workflow() {
        let repo = setup().await;
        let labels = repo.get_labels("wf_default").await.unwrap();
        assert_eq!(labels.len(), 6);
        assert_eq!(labels[0].name, "Backlog");
        assert_eq!(labels[5].name, "Blocked");
    }

    #[tokio::test]
    async fn test_create_workflow_with_labels() {
        let repo = setup().await;
        let wf = repo.create("My Custom", false).await.unwrap();
        assert!(!wf.id.is_empty());

        repo.add_label(&wf.id, "Open", "#3b82f6", "not_started", 0).await.unwrap();
        repo.add_label(&wf.id, "Closed", "#22c55e", "done", 1).await.unwrap();

        let labels = repo.get_labels(&wf.id).await.unwrap();
        assert_eq!(labels.len(), 2);
    }

    #[tokio::test]
    async fn test_update_label() {
        let repo = setup().await;
        let updated = repo.update_label("sl_todo", Some("To Do"), Some("#60a5fa"), None, None).await.unwrap();
        assert_eq!(updated.name, "To Do");
        assert_eq!(updated.color, "#60a5fa");
    }

    #[tokio::test]
    async fn test_delete_label() {
        let repo = setup().await;
        let deleted = repo.delete_label("sl_backlog").await.unwrap();
        assert!(deleted);
        let labels = repo.get_labels("wf_default").await.unwrap();
        assert_eq!(labels.len(), 5);
    }

    #[tokio::test]
    async fn test_delete_workflow_cascades_labels() {
        let repo = setup().await;
        let wf = repo.create("Temp", false).await.unwrap();
        repo.add_label(&wf.id, "X", "#fff", "active", 0).await.unwrap();
        repo.delete(&wf.id).await.unwrap();
        let labels = repo.get_labels(&wf.id).await.unwrap();
        assert!(labels.is_empty());
    }

    #[tokio::test]
    async fn test_duplicate_workflow() {
        let repo = setup().await;
        let dup = repo.duplicate("wf_default", "My Copy").await.unwrap();
        assert_ne!(dup.id, "wf_default");
        assert_eq!(dup.name, "My Copy");
        let labels = repo.get_labels(&dup.id).await.unwrap();
        assert_eq!(labels.len(), 6);
    }

    #[tokio::test]
    async fn test_get_effective_workflow_fallback() {
        let repo = setup().await;
        // No project workflow → falls back to global default
        let labels = repo.get_effective_labels(None).await.unwrap();
        assert_eq!(labels.len(), 6);
    }

    #[tokio::test]
    async fn test_resolve_status_group() {
        let repo = setup().await;
        let group = repo.get_label("sl_done").await.unwrap().unwrap();
        assert_eq!(group.status_group, "done");
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p storage -E 'test(status_workflow)' 2>&1 | tail -20`
Expected: Compilation error — `StatusWorkflowRepo` doesn't exist yet

**Step 3: Implement the repository**

Write the full `crates/storage/src/repos/status_workflow.rs`:

```rust
use crate::error::{OptionExt, StorageError};
use crate::rows::status::{StatusLabelRow, StatusWorkflowRow};

#[derive(Debug, Clone)]
pub struct StatusWorkflowRepo {
    pool: sqlx::SqlitePool,
}

impl StatusWorkflowRepo {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }

    // ── Workflow CRUD ─────────────────────────────────────

    pub async fn get(&self, id: &str) -> Result<Option<StatusWorkflowRow>, StorageError> {
        Ok(sqlx::query_as::<_, StatusWorkflowRow>("SELECT * FROM status_workflows WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn get_global_default(&self) -> Result<Option<StatusWorkflowRow>, StorageError> {
        Ok(sqlx::query_as::<_, StatusWorkflowRow>(
            "SELECT * FROM status_workflows WHERE is_global_default = 1 LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn list_templates(&self) -> Result<Vec<StatusWorkflowRow>, StorageError> {
        Ok(sqlx::query_as::<_, StatusWorkflowRow>(
            "SELECT * FROM status_workflows WHERE is_template = 1 ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn list_all(&self) -> Result<Vec<StatusWorkflowRow>, StorageError> {
        Ok(sqlx::query_as::<_, StatusWorkflowRow>(
            "SELECT * FROM status_workflows ORDER BY is_global_default DESC, name",
        )
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn create(
        &self,
        name: &str,
        is_template: bool,
    ) -> Result<StatusWorkflowRow, StorageError> {
        let id = format!("wf_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        sqlx::query(
            "INSERT INTO status_workflows (id, name, is_template) VALUES (?1, ?2, ?3)",
        )
        .bind(&id)
        .bind(name)
        .bind(is_template)
        .execute(&self.pool)
        .await?;
        self.get(&id).await?.ok_or_not_found("workflow")
    }

    pub async fn update_name(&self, id: &str, name: &str) -> Result<StatusWorkflowRow, StorageError> {
        sqlx::query("UPDATE status_workflows SET name = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?2")
            .bind(name)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.get(id).await?.ok_or_not_found("workflow")
    }

    pub async fn delete(&self, id: &str) -> Result<bool, StorageError> {
        let r = sqlx::query("DELETE FROM status_workflows WHERE id = ?1 AND is_global_default = 0")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(r.rows_affected() > 0)
    }

    pub async fn duplicate(
        &self,
        source_id: &str,
        new_name: &str,
    ) -> Result<StatusWorkflowRow, StorageError> {
        let source = self.get(source_id).await?.ok_or_not_found("source workflow")?;
        let new_wf = self.create(new_name, source.is_template).await?;
        let labels = self.get_labels(source_id).await?;
        for label in &labels {
            self.add_label(&new_wf.id, &label.name, &label.color, &label.status_group, label.position)
                .await?;
        }
        Ok(new_wf)
    }

    // ── Label CRUD ────────────────────────────────────────

    pub async fn get_label(&self, id: &str) -> Result<Option<StatusLabelRow>, StorageError> {
        Ok(sqlx::query_as::<_, StatusLabelRow>("SELECT * FROM status_labels WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn get_labels(&self, workflow_id: &str) -> Result<Vec<StatusLabelRow>, StorageError> {
        Ok(sqlx::query_as::<_, StatusLabelRow>(
            "SELECT * FROM status_labels WHERE workflow_id = ?1 ORDER BY position",
        )
        .bind(workflow_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn add_label(
        &self,
        workflow_id: &str,
        name: &str,
        color: &str,
        status_group: &str,
        position: i32,
    ) -> Result<StatusLabelRow, StorageError> {
        let id = format!("sl_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        sqlx::query(
            "INSERT INTO status_labels (id, workflow_id, name, color, status_group, position) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&id)
        .bind(workflow_id)
        .bind(name)
        .bind(color)
        .bind(status_group)
        .bind(position)
        .execute(&self.pool)
        .await?;
        self.get_label(&id).await?.ok_or_not_found("label")
    }

    pub async fn update_label(
        &self,
        id: &str,
        name: Option<&str>,
        color: Option<&str>,
        status_group: Option<&str>,
        position: Option<i32>,
    ) -> Result<StatusLabelRow, StorageError> {
        sqlx::query(
            "UPDATE status_labels SET \
             name = COALESCE(?2, name), \
             color = COALESCE(?3, color), \
             status_group = COALESCE(?4, status_group), \
             position = COALESCE(?5, position) \
             WHERE id = ?1",
        )
        .bind(id)
        .bind(name)
        .bind(color)
        .bind(status_group)
        .bind(position)
        .execute(&self.pool)
        .await?;
        self.get_label(id).await?.ok_or_not_found("label")
    }

    pub async fn delete_label(&self, id: &str) -> Result<bool, StorageError> {
        let r = sqlx::query("DELETE FROM status_labels WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(r.rows_affected() > 0)
    }

    pub async fn reorder_labels(
        &self,
        workflow_id: &str,
        label_ids: &[String],
    ) -> Result<(), StorageError> {
        for (i, id) in label_ids.iter().enumerate() {
            sqlx::query("UPDATE status_labels SET position = ?1 WHERE id = ?2 AND workflow_id = ?3")
                .bind(i as i32)
                .bind(id)
                .bind(workflow_id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    // ── Effective workflow resolution ─────────────────────

    /// Get the effective labels for a project. If project_workflow_id is None,
    /// falls back to the global default workflow.
    pub async fn get_effective_labels(
        &self,
        project_workflow_id: Option<&str>,
    ) -> Result<Vec<StatusLabelRow>, StorageError> {
        let wf_id = match project_workflow_id {
            Some(id) => id.to_string(),
            None => {
                let default = self.get_global_default().await?.ok_or_not_found("global default workflow")?;
                default.id
            }
        };
        self.get_labels(&wf_id).await
    }

    /// Find the first label in a given status_group for a workflow.
    /// Used by AI: "mark as done" → find first label where group = "done".
    pub async fn find_label_by_group(
        &self,
        workflow_id: &str,
        status_group: &str,
    ) -> Result<Option<StatusLabelRow>, StorageError> {
        Ok(sqlx::query_as::<_, StatusLabelRow>(
            "SELECT * FROM status_labels WHERE workflow_id = ?1 AND status_group = ?2 ORDER BY position LIMIT 1",
        )
        .bind(workflow_id)
        .bind(status_group)
        .fetch_optional(&self.pool)
        .await?)
    }
}
```

**Step 4: Register in repos/mod.rs**

Add module declaration and re-export:

```rust
pub mod status_workflow;
pub use status_workflow::StatusWorkflowRepo;
```

Add field to `Repos` struct:

```rust
pub status_workflows: StatusWorkflowRepo,
```

Add initialization in `Repos::from_pool`:

```rust
status_workflows: StatusWorkflowRepo::new(db.clone()),
```

**Step 5: Run tests**

Run: `cargo nextest run -p storage -E 'test(status_workflow)' 2>&1 | tail -20`
Expected: All tests pass

**Step 6: Commit**

```bash
git add crates/storage/src/repos/status_workflow.rs crates/storage/src/repos/mod.rs
git commit -m "feat(storage): add StatusWorkflowRepo with CRUD, templates, effective resolution"
```

---

## Task 4: Update ActionRepo for Status Labels

**Files:**
- Modify: `crates/storage/src/repos/action_repo.rs`

**Step 1: Write failing tests**

Add to the existing test module in `action_repo.rs`:

```rust
#[tokio::test]
async fn test_action_with_status_label_id() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = ActionRepo::new(pool.inner().clone());
    let mut row = ActionRow {
        id: "test-sl".into(),
        title: "Test with label".into(),
        description: None,
        area_id: "default".into(),
        project_id: None,
        key_result_id: None,
        parent_id: None,
        priority: None,
        due_date: None,
        tags: vec![],
        status: "todo".into(),
        status_label_id: Some("sl_todo".into()),
        position: 0,
        focused_at: None,
        focus_deadline: None,
        focus_expired_count: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        completed_at: None,
        total_tracked_secs: 0,
        estimated_minutes: None,
        calendar_event_uid: None,
        last_reminded_at: None,
        recurrence_rule: None,
        recurrence_parent_id: None,
        is_template: false,
        next_instance_date: None,
    };
    let inserted = repo.add(&row).await.unwrap();
    assert_eq!(inserted.status_label_id, Some("sl_todo".into()));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p storage -E 'test(action_with_status_label)' 2>&1 | tail -20`
Expected: FAIL — ActionRow doesn't have `status_label_id` field yet (or SQL insert doesn't include it)

**Step 3: Update ActionRepo SQL queries**

In `action_repo.rs`, update these methods to include `status_label_id` and `position`:

1. **`add()`** — Add `status_label_id` and `position` to INSERT column list and bind values
2. **`update()`** — Add `status_label_id = COALESCE(?N, status_label_id)` and `position = COALESCE(?N, position)` to UPDATE SET clause
3. **`ActionPatch`** — Add `pub status_label_id: Option<Option<String>>` and `pub position: Option<i32>`

Also update `ActionSummary` to count by status_group (via JOIN) instead of hardcoded strings. Add a new method:

```rust
pub async fn summary_by_group(&self) -> Result<HashMap<String, i64>, StorageError> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT sl.status_group, COUNT(*) as cnt \
         FROM actions a \
         JOIN status_labels sl ON a.status_label_id = sl.id \
         WHERE a.is_template = 0 \
         GROUP BY sl.status_group"
    )
    .fetch_all(&self.pool)
    .await?;
    Ok(rows.into_iter().collect())
}
```

**Step 4: Run tests**

Run: `cargo nextest run -p storage -E 'test(action)' 2>&1 | tail -30`
Expected: All pass

**Step 5: Commit**

```bash
git add crates/storage/src/repos/action_repo.rs
git commit -m "feat(storage): update ActionRepo to support status_label_id and position"
```

---

## Task 5: IPC Types — StatusWorkflow and StatusLabel Responses

**Files:**
- Modify: `crates/desktop-shared/src/commands.rs`

**Step 1: Add response and param types**

Add to `crates/desktop-shared/src/commands.rs`:

```rust
// ── Status Workflows ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusWorkflowResponse {
    pub id: String,
    pub name: String,
    pub is_template: bool,
    pub is_global_default: bool,
    pub labels: Vec<StatusLabelResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusLabelResponse {
    pub id: String,
    pub workflow_id: String,
    pub name: String,
    pub color: String,
    pub status_group: String,
    pub position: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowCreateParams {
    pub name: String,
    pub is_template: Option<bool>,
    pub source_workflow_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelCreateParams {
    pub workflow_id: String,
    pub name: String,
    pub color: String,
    pub status_group: String,
    pub position: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelUpdateParams {
    pub id: String,
    pub name: Option<String>,
    pub color: Option<String>,
    pub status_group: Option<String>,
    pub position: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelReorderParams {
    pub workflow_id: String,
    pub label_ids: Vec<String>,
}
```

Also update `TaskResponse` to include:

```rust
pub status_label_id: Option<String>,
pub status_label: Option<StatusLabelResponse>,
```

And update `TaskCreateParams` to include:

```rust
pub status_label_id: Option<String>,
```

And update `TaskUpdateParams` to include:

```rust
pub status_label_id: Option<Option<String>>,
pub position: Option<i32>,
```

And update `ProjectResponse` to include:

```rust
pub workflow_id: Option<String>,
```

**Step 2: Build to verify**

Run: `cargo build -p desktop-shared 2>&1 | head -20`
Expected: Compiles

**Step 3: Commit**

```bash
git add crates/desktop-shared/src/commands.rs
git commit -m "feat(desktop-shared): add StatusWorkflow/Label IPC types, update Task/Project types"
```

---

## Task 6: Tauri Commands — Workflow & Label CRUD

**Files:**
- Create: `crates/desktop/src/commands/workflows.rs`
- Modify: `crates/desktop/src/commands/mod.rs` (register new module)
- Modify: `crates/desktop/src/commands/tasks.rs` (update converters)
- Modify: `crates/desktop/src/lib.rs` (register commands with Tauri)

**Step 1: Create workflow commands**

Create `crates/desktop/src/commands/workflows.rs`:

```rust
use desktop_shared::commands::*;
use tauri::State;
use crate::state::AppState;

fn workflow_to_response(
    wf: storage::rows::status::StatusWorkflowRow,
    labels: Vec<storage::rows::status::StatusLabelRow>,
) -> StatusWorkflowResponse {
    StatusWorkflowResponse {
        id: wf.id,
        name: wf.name,
        is_template: wf.is_template,
        is_global_default: wf.is_global_default,
        labels: labels.into_iter().map(label_to_response).collect(),
    }
}

fn label_to_response(l: storage::rows::status::StatusLabelRow) -> StatusLabelResponse {
    StatusLabelResponse {
        id: l.id,
        workflow_id: l.workflow_id,
        name: l.name,
        color: l.color,
        status_group: l.status_group,
        position: l.position,
    }
}

#[tauri::command]
pub async fn workflow_list(state: State<'_, AppState>) -> Result<Vec<StatusWorkflowResponse>, String> {
    let workflows = state.repos.status_workflows.list_all().await.map_err(|e| e.to_string())?;
    let mut result = Vec::new();
    for wf in workflows {
        let labels = state.repos.status_workflows.get_labels(&wf.id).await.map_err(|e| e.to_string())?;
        result.push(workflow_to_response(wf, labels));
    }
    Ok(result)
}

#[tauri::command]
pub async fn workflow_get(id: String, state: State<'_, AppState>) -> Result<Option<StatusWorkflowResponse>, String> {
    let wf = state.repos.status_workflows.get(&id).await.map_err(|e| e.to_string())?;
    match wf {
        Some(wf) => {
            let labels = state.repos.status_workflows.get_labels(&wf.id).await.map_err(|e| e.to_string())?;
            Ok(Some(workflow_to_response(wf, labels)))
        }
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn workflow_get_effective(project_id: Option<String>, state: State<'_, AppState>) -> Result<Vec<StatusLabelResponse>, String> {
    let workflow_id = match project_id {
        Some(pid) => {
            let proj = state.repos.projects.get(&pid).await.map_err(|e| e.to_string())?;
            proj.and_then(|p| p.workflow_id)
        }
        None => None,
    };
    let labels = state.repos.status_workflows
        .get_effective_labels(workflow_id.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    Ok(labels.into_iter().map(label_to_response).collect())
}

#[tauri::command]
pub async fn workflow_create(params: WorkflowCreateParams, state: State<'_, AppState>) -> Result<StatusWorkflowResponse, String> {
    let wf = match params.source_workflow_id {
        Some(source_id) => {
            state.repos.status_workflows.duplicate(&source_id, &params.name).await.map_err(|e| e.to_string())?
        }
        None => {
            state.repos.status_workflows.create(&params.name, params.is_template.unwrap_or(false)).await.map_err(|e| e.to_string())?
        }
    };
    let labels = state.repos.status_workflows.get_labels(&wf.id).await.map_err(|e| e.to_string())?;
    Ok(workflow_to_response(wf, labels))
}

#[tauri::command]
pub async fn workflow_delete(id: String, state: State<'_, AppState>) -> Result<bool, String> {
    state.repos.status_workflows.delete(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn label_create(params: LabelCreateParams, state: State<'_, AppState>) -> Result<StatusLabelResponse, String> {
    let label = state.repos.status_workflows
        .add_label(&params.workflow_id, &params.name, &params.color, &params.status_group, params.position.unwrap_or(0))
        .await
        .map_err(|e| e.to_string())?;
    Ok(label_to_response(label))
}

#[tauri::command]
pub async fn label_update(params: LabelUpdateParams, state: State<'_, AppState>) -> Result<StatusLabelResponse, String> {
    let label = state.repos.status_workflows
        .update_label(&params.id, params.name.as_deref(), params.color.as_deref(), params.status_group.as_deref(), params.position)
        .await
        .map_err(|e| e.to_string())?;
    Ok(label_to_response(label))
}

#[tauri::command]
pub async fn label_delete(id: String, state: State<'_, AppState>) -> Result<bool, String> {
    state.repos.status_workflows.delete_label(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn label_reorder(params: LabelReorderParams, state: State<'_, AppState>) -> Result<(), String> {
    state.repos.status_workflows
        .reorder_labels(&params.workflow_id, &params.label_ids)
        .await
        .map_err(|e| e.to_string())
}
```

**Step 2: Update task commands to include status_label**

In `crates/desktop/src/commands/tasks.rs`, update the `action_to_task()` converter to populate `status_label_id` and `status_label` fields. Also update `task_create` to accept and use `status_label_id`, and `task_update` to accept `status_label_id` and `position`.

**Step 3: Register commands in Tauri**

In `crates/desktop/src/lib.rs`, add the new commands to the `invoke_handler`:

```rust
workflow_list, workflow_get, workflow_get_effective, workflow_create, workflow_delete,
label_create, label_update, label_delete, label_reorder,
```

**Step 4: Build to verify**

Run: `cargo build -p desktop 2>&1 | tail -20`
Expected: Compiles

**Step 5: Commit**

```bash
git add crates/desktop/src/commands/workflows.rs crates/desktop/src/commands/mod.rs crates/desktop/src/commands/tasks.rs crates/desktop/src/lib.rs
git commit -m "feat(desktop): add workflow/label Tauri commands, update task converters"
```

---

## Task 7: Dev-API Endpoints for Workflows

**Files:**
- Modify: `crates/dev-api/src/main.rs`

**Step 1: Add workflow endpoints**

Add Axum route handlers for:

- `POST /api/workflow_list` → calls `repos.status_workflows.list_all()` + `get_labels()`
- `POST /api/workflow_get` → `{ id }` → single workflow with labels
- `POST /api/workflow_get_effective` → `{ projectId? }` → effective labels for project
- `POST /api/workflow_create` → `WorkflowCreateParams` → create/duplicate
- `POST /api/workflow_delete` → `{ id }` → delete
- `POST /api/label_create` → `LabelCreateParams` → add label
- `POST /api/label_update` → `LabelUpdateParams` → update label
- `POST /api/label_delete` → `{ id }` → delete label
- `POST /api/label_reorder` → `LabelReorderParams` → reorder

Also update the `action_to_task()` helper to include `status_label_id` and `status_label`.

**Step 2: Build and verify**

Run: `cargo build -p dev-api 2>&1 | tail -20`
Expected: Compiles

**Step 3: Commit**

```bash
git add crates/dev-api/src/main.rs
git commit -m "feat(dev-api): add workflow/label REST endpoints"
```

---

## Task 8: Frontend Types & API Hooks

**Files:**
- Modify: `desktop-ui/src/lib/types.ts`
- Create: `desktop-ui/src/hooks/useWorkflows.ts`

**Step 1: Add TypeScript types**

In `desktop-ui/src/lib/types.ts`, add:

```typescript
// ── Status Workflows ──────────────────────────────────

export interface StatusWorkflow {
  id: string;
  name: string;
  isTemplate: boolean;
  isGlobalDefault: boolean;
  labels: StatusLabel[];
}

export interface StatusLabel {
  id: string;
  workflowId: string;
  name: string;
  color: string;
  statusGroup: "not_started" | "active" | "done" | "stuck";
  position: number;
}
```

Update the `Task` interface to add:

```typescript
statusLabelId: string | null;
statusLabel: StatusLabel | null;
```

**Step 2: Create useWorkflows hook**

Create `desktop-ui/src/hooks/useWorkflows.ts`:

```typescript
import { useQuery } from "./useQuery";
import { useMutation } from "./useMutation";
import type { StatusLabel, StatusWorkflow } from "../lib/types";

/** Fetch all workflows (including templates) */
export function useWorkflows() {
  return useQuery<StatusWorkflow[]>("workflow_list", undefined, []);
}

/** Fetch effective labels for a project (or global default if null) */
export function useEffectiveLabels(projectId: string | null) {
  return useQuery<StatusLabel[]>(
    "workflow_get_effective",
    { projectId },
    [],
  );
}

/** Mutations for workflow management */
export function useWorkflowMutations() {
  const create = useMutation<StatusWorkflow, {
    name: string;
    isTemplate?: boolean;
    sourceWorkflowId?: string;
  }>("workflow_create", "params");

  const deleteWf = useMutation<boolean, { id: string }>("workflow_delete");

  const createLabel = useMutation<StatusLabel, {
    workflowId: string;
    name: string;
    color: string;
    statusGroup: string;
    position?: number;
  }>("label_create", "params");

  const updateLabel = useMutation<StatusLabel, {
    id: string;
    name?: string;
    color?: string;
    statusGroup?: string;
    position?: number;
  }>("label_update", "params");

  const deleteLabel = useMutation<boolean, { id: string }>("label_delete");

  const reorderLabels = useMutation<void, {
    workflowId: string;
    labelIds: string[];
  }>("label_reorder", "params");

  return { create, delete: deleteWf, createLabel, updateLabel, deleteLabel, reorderLabels };
}
```

**Step 3: Commit**

```bash
git add desktop-ui/src/lib/types.ts desktop-ui/src/hooks/useWorkflows.ts
git commit -m "feat(ui): add StatusWorkflow/Label types and useWorkflows hook"
```

---

## Task 9: Dynamic Status Select in TaskRow

**Files:**
- Modify: `desktop-ui/src/components/tasks/TaskRow.tsx`
- Modify: `desktop-ui/src/components/tasks/TaskTableContext.tsx`
- Modify: `desktop-ui/src/components/views/MainApp.tsx`
- Modify: `desktop-ui/src/components/ui/Badge.tsx`

**Step 1: Update TaskTableContext to provide status labels**

In `TaskTableContext.tsx`, add `statusLabels: StatusLabel[]` to the context type and provider.

**Step 2: Update MainApp to fetch and provide effective labels**

In `MainApp.tsx`, add `useEffectiveLabels(null)` call and pass to context.

**Step 3: Update TaskRow status column**

Replace the hardcoded STATUS_OPTIONS with dynamic options from context:

```typescript
const { statusLabels } = useTaskTable();
const statusOptions = statusLabels.map((sl) => ({
  value: sl.id,
  label: sl.name,
}));
```

Update the InlineSelect for status to:
- Use `task.statusLabelId` as value instead of `task.status`
- Call `onUpdate({ id: task.id, statusLabelId: selectedLabelId })` on change

**Step 4: Update Badge to support dynamic status colors**

In `Badge.tsx`, update the status variant to accept a `color` prop (hex from status label) instead of looking up from a hardcoded map. When a `color` prop is provided, use it directly with inline styles.

**Step 5: Build and verify**

Run: `cd desktop-ui && bun run build 2>&1 | tail -20`
Expected: Builds successfully

**Step 6: Commit**

```bash
git add desktop-ui/src/components/tasks/TaskRow.tsx desktop-ui/src/components/tasks/TaskTableContext.tsx desktop-ui/src/components/views/MainApp.tsx desktop-ui/src/components/ui/Badge.tsx
git commit -m "feat(ui): dynamic status labels in TaskRow with colors from workflow"
```

---

## Task 10: Dynamic Kanban Columns

**Files:**
- Modify: `desktop-ui/src/components/tasks/KanbanBoard.tsx`

**Step 1: Replace hardcoded COLUMNS**

Remove the `COLUMNS` constant. Instead, receive `statusLabels` via props or context. Generate columns dynamically:

```typescript
const columns = statusLabels.map((sl) => ({
  key: sl.id,
  label: sl.name,
  color: sl.color,
}));
```

**Step 2: Update task grouping**

Group tasks by `task.statusLabelId` instead of `task.status`:

```typescript
const grouped = new Map<string, Task[]>();
for (const col of columns) {
  grouped.set(col.key, []);
}
for (const task of tasks) {
  const key = task.statusLabelId ?? columns[0]?.key;
  grouped.get(key)?.push(task);
}
```

**Step 3: Update column header styling**

Use `sl.color` as the accent bar color (inline style) instead of Tailwind classes.

**Step 4: Build and verify**

Run: `cd desktop-ui && bun run build 2>&1 | tail -20`
Expected: Builds successfully

**Step 5: Commit**

```bash
git add desktop-ui/src/components/tasks/KanbanBoard.tsx
git commit -m "feat(ui): dynamic kanban columns from status workflow labels"
```

---

## Task 11: Update feature-todo Tool for Status Labels

**Files:**
- Modify: `crates/feature-todo/src/types.rs` (ActionStatus → support label IDs)
- Modify: `crates/feature-todo/src/tool/actions/add.rs` (use status_label_id)
- Modify: `crates/feature-todo/src/tool/actions/update.rs` (support status_label_id changes)
- Modify: `crates/feature-todo/src/tool/actions/list.rs` (filter by status_group)

**Step 1: Update Action domain type**

In `types.rs`, add:

```rust
pub status_label_id: Option<String>,
pub position: i32,
```

Keep `ActionStatus` enum for backward compatibility with AI chat commands — add a mapping method:

```rust
impl ActionStatus {
    /// Map legacy status to status_group for AI queries.
    pub fn to_group(&self) -> &'static str {
        match self {
            Self::Todo => "not_started",
            Self::Doing => "active",
            Self::Done => "done",
            Self::Archived => "done",
        }
    }
}
```

**Step 2: Update add action handler**

In `add.rs`, when creating a task:
- If `status_label_id` is provided, use it
- Otherwise, resolve from the project's workflow (first `not_started` label)

**Step 3: Update list/summary handlers**

In `list.rs`, update `handle_summary` to use `summary_by_group()` from ActionRepo.

**Step 4: Run tests**

Run: `cargo nextest run -p feature-todo 2>&1 | tail -20`
Expected: All pass

**Step 5: Commit**

```bash
git add crates/feature-todo/src/
git commit -m "feat(todo): support status_label_id in Action type and tool handlers"
```

---

## Task 12: Project Workflow Assignment UI

**Files:**
- Modify: `desktop-ui/src/components/views/MainApp.tsx` or project settings component
- Create: `desktop-ui/src/components/tasks/WorkflowPicker.tsx`

**Step 1: Create WorkflowPicker component**

A dropdown that shows available workflows (global default + templates + custom) and lets the user assign one to a project.

```typescript
interface WorkflowPickerProps {
  projectId: string;
  currentWorkflowId: string | null;
  onSelect: (workflowId: string | null) => void;
}
```

**Step 2: Wire to project update API**

Update `ProjectUpdateParams` to include `workflow_id` in desktop-shared. Wire the picker to call `project_update` with the selected workflow_id.

**Step 3: Build and verify**

Run: `cd desktop-ui && bun run build 2>&1 | tail -20`
Expected: Builds

**Step 4: Commit**

```bash
git add desktop-ui/src/components/tasks/WorkflowPicker.tsx crates/desktop-shared/src/commands.rs
git commit -m "feat(ui): WorkflowPicker for per-project workflow assignment"
```

---

## Task 13: Integration Test — Full Workflow Lifecycle

**Files:**
- Create: `crates/storage/src/repos/tests/status_workflow_integration.rs` (or add to existing tests module)

**Step 1: Write integration test**

```rust
#[tokio::test]
async fn test_full_workflow_lifecycle() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = Repos::from_pool(&pool);

    // 1. Global default exists with 6 labels
    let default = repos.status_workflows.get_global_default().await.unwrap().unwrap();
    let labels = repos.status_workflows.get_labels(&default.id).await.unwrap();
    assert_eq!(labels.len(), 6);

    // 2. Create custom workflow by duplicating
    let custom = repos.status_workflows.duplicate(&default.id, "My Project").await.unwrap();
    let custom_labels = repos.status_workflows.get_labels(&custom.id).await.unwrap();
    assert_eq!(custom_labels.len(), 6);

    // 3. Add a label
    repos.status_workflows.add_label(&custom.id, "Testing", "#8b5cf6", "active", 4).await.unwrap();
    let updated_labels = repos.status_workflows.get_labels(&custom.id).await.unwrap();
    assert_eq!(updated_labels.len(), 7);

    // 4. Create task with status_label_id
    let todo_label = &custom_labels[1]; // "Todo"
    let action = repos.actions.add(&ActionRow {
        id: "task-wf-test".into(),
        status: "todo".into(),
        status_label_id: Some(todo_label.id.clone()),
        position: 0,
        // ... other required fields with defaults
        ..Default::default()
    }).await.unwrap();
    assert_eq!(action.status_label_id, Some(todo_label.id.clone()));

    // 5. Effective labels fallback
    let effective = repos.status_workflows.get_effective_labels(None).await.unwrap();
    assert_eq!(effective.len(), 6); // global default
}
```

**Step 2: Run test**

Run: `cargo nextest run -p storage -E 'test(full_workflow_lifecycle)' 2>&1 | tail -20`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/storage/src/repos/tests/
git commit -m "test(storage): integration test for full workflow lifecycle"
```

---

## Task 14: Lint, Format, Final Verification

**Step 1: Run all checks**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features
cargo nextest run --workspace
cd desktop-ui && bun run lint:fix && bun run build
```

**Step 2: Fix any issues found**

**Step 3: Final commit if needed**

```bash
git commit -m "chore: lint fixes for Phase 1 status system"
```

---

## Summary

| Task | Scope | Key Files |
|------|-------|-----------|
| 1 | Migration SQL | `crates/storage/migrations/004_status_workflows.sql` |
| 2 | Row structs | `crates/storage/src/rows/status.rs`, `action.rs` |
| 3 | StatusWorkflowRepo | `crates/storage/src/repos/status_workflow.rs`, `mod.rs` |
| 4 | ActionRepo updates | `crates/storage/src/repos/action_repo.rs` |
| 5 | IPC types | `crates/desktop-shared/src/commands.rs` |
| 6 | Tauri commands | `crates/desktop/src/commands/workflows.rs`, `tasks.rs` |
| 7 | Dev-API endpoints | `crates/dev-api/src/main.rs` |
| 8 | Frontend types & hooks | `desktop-ui/src/lib/types.ts`, `useWorkflows.ts` |
| 9 | Dynamic status select | `TaskRow.tsx`, `TaskTableContext.tsx`, `Badge.tsx` |
| 10 | Dynamic kanban columns | `KanbanBoard.tsx` |
| 11 | feature-todo updates | `crates/feature-todo/src/` |
| 12 | Workflow picker UI | `WorkflowPicker.tsx` |
| 13 | Integration test | `crates/storage/src/repos/tests/` |
| 14 | Lint & verify | All |
