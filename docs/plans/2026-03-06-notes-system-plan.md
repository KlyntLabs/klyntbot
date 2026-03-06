# Notes System Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a full-featured notes system (rich editor, notebooks, wiki-links, graph, search, version history) integrated into the klyntbot desktop app.

**Architecture:** New `feature-notes` crate following the `feature-todo` pattern (FeaturePackage with tools + migrations + config). SQLite storage via `StoragePool`. React frontend with TipTap v3 editor ported from HelixNotes. Three phases: Core, Knowledge, Integration.

**Tech Stack:** Rust (sqlx, uuid, chrono), React 19 (TipTap v3, Lowlight, KaTeX), Tailwind v4 glass design system, existing useQuery/useMutation hooks.

**Reference:** Design doc at `docs/plans/2026-03-06-notes-system-design.md`. HelixNotes source at `../helixnotes/` (TipTap config reference).

---

## Phase 1: Core (Backend + Frontend CRUD)

### Task 1: Create feature-notes crate scaffold

**Files:**
- Create: `crates/feature-notes/Cargo.toml`
- Create: `crates/feature-notes/src/lib.rs`
- Modify: `Cargo.toml` (workspace members)

**Step 1: Create Cargo.toml**

```toml
[package]
name = "feature-notes"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
common.workspace = true
tools-core.workspace = true
storage.workspace = true
async-trait.workspace = true
serde = { workspace = true }
serde_json.workspace = true
tokio.workspace = true
tracing.workspace = true
chrono = { workspace = true }
uuid = { workspace = true }
sqlx.workspace = true

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "test-util"] }
```

**Step 2: Create minimal lib.rs**

```rust
//! feature-notes: Notes and knowledge management feature package for klyntbot.

pub mod models;
pub mod repo;

use async_trait::async_trait;
use common::Result;
use serde_json::Value;
use tools_core::{FeatureMigration, FeaturePackage, HealthStatus};

pub struct NotesFeature {
    repo: repo::NoteRepo,
}

impl NotesFeature {
    pub fn new(repo: repo::NoteRepo) -> Self {
        Self { repo }
    }

    pub fn migration_sql() -> &'static str {
        include_str!("../migrations/001_create_notes.sql")
    }
}

#[async_trait]
impl FeaturePackage for NotesFeature {
    fn name(&self) -> &str {
        "notes"
    }

    fn tools(&self) -> Vec<tools_core::DynTool> {
        vec![] // Added in later tasks
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature_name: "notes".to_string(),
            version: 1,
            description: "Create notes core tables (notebooks, notes, tags, links, versions)"
                .to_string(),
            sql: Self::migration_sql().to_string(),
        }]
    }

    fn config_key(&self) -> &str {
        "notes"
    }

    fn default_config(&self) -> Value {
        serde_json::json!({
            "maxVersionsPerNote": 50,
            "versionCooldownMinutes": 5
        })
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        match self.repo.count().await {
            Ok(_) => Ok(HealthStatus::Healthy),
            Err(e) => Ok(HealthStatus::Unhealthy(format!("DB check failed: {e}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_sql_not_empty() {
        let sql = NotesFeature::migration_sql();
        assert!(!sql.is_empty());
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS notebooks"));
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS notes"));
    }
}
```

**Step 3: Add to workspace Cargo.toml**

Add `"crates/feature-notes"` to the `[workspace] members` array.

**Step 4: Create empty model and repo files**

Create `crates/feature-notes/src/models.rs` and `crates/feature-notes/src/repo.rs` with placeholder module docs so lib.rs compiles.

**Step 5: Run build to verify scaffold**

Run: `cargo build -p feature-notes`
Expected: PASS (compiles, migration file not yet created so test will fail — that's fine)

**Step 6: Commit**

```bash
git add crates/feature-notes/ Cargo.toml
git commit -m "feat(notes): scaffold feature-notes crate"
```

---

### Task 2: Migration SQL + Models

**Files:**
- Create: `crates/feature-notes/migrations/001_create_notes.sql`
- Modify: `crates/feature-notes/src/models.rs`

**Step 1: Write migration SQL**

Create `crates/feature-notes/migrations/001_create_notes.sql`:

```sql
-- Feature migration: notes tables
CREATE TABLE IF NOT EXISTS notebooks (
    id          TEXT PRIMARY KEY,
    parent_id   TEXT REFERENCES notebooks(id) ON DELETE SET NULL,
    title       TEXT NOT NULL,
    icon        TEXT,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_notebooks_parent_id ON notebooks(parent_id);

CREATE TABLE IF NOT EXISTS notes (
    id          TEXT PRIMARY KEY,
    notebook_id TEXT REFERENCES notebooks(id) ON DELETE SET NULL,
    title       TEXT NOT NULL,
    body        TEXT NOT NULL DEFAULT '',
    body_html   TEXT,
    pinned      INTEGER NOT NULL DEFAULT 0,
    archived    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_notes_notebook_id ON notes(notebook_id);
CREATE INDEX IF NOT EXISTS idx_notes_pinned ON notes(pinned) WHERE pinned = 1;
CREATE INDEX IF NOT EXISTS idx_notes_updated_at ON notes(updated_at);

CREATE TABLE IF NOT EXISTS note_tags (
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    tag     TEXT NOT NULL,
    PRIMARY KEY (note_id, tag)
);

CREATE INDEX IF NOT EXISTS idx_note_tags_tag ON note_tags(tag);

CREATE TABLE IF NOT EXISTS note_links (
    source_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    target_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    PRIMARY KEY (source_id, target_id),
    CHECK (source_id != target_id)
);

CREATE INDEX IF NOT EXISTS idx_note_links_target ON note_links(target_id);

CREATE TABLE IF NOT EXISTS note_entity_mentions (
    note_id     TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    entity_type TEXT NOT NULL,
    entity_id   TEXT NOT NULL,
    PRIMARY KEY (note_id, entity_type, entity_id)
);

CREATE INDEX IF NOT EXISTS idx_note_entity_mentions_entity
    ON note_entity_mentions(entity_type, entity_id);

CREATE TABLE IF NOT EXISTS note_versions (
    id         TEXT PRIMARY KEY,
    note_id    TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    body       TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_note_versions_note_id ON note_versions(note_id);
```

**Step 2: Define models**

Write `crates/feature-notes/src/models.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notebook {
    pub id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub icon: Option<String>,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    pub notebook_id: Option<String>,
    pub title: String,
    pub body: String,
    pub body_html: Option<String>,
    pub pinned: bool,
    pub archived: bool,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteVersion {
    pub id: String,
    pub note_id: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteLink {
    pub source_id: String,
    pub target_id: String,
}

/// SQLite row for notebooks (maps 1:1 to table).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NotebookRow {
    pub id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub icon: Option<String>,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

/// SQLite row for notes (maps 1:1 to table).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NoteRow {
    pub id: String,
    pub notebook_id: Option<String>,
    pub title: String,
    pub body: String,
    pub body_html: Option<String>,
    pub pinned: i32,
    pub archived: i32,
    pub created_at: String,
    pub updated_at: String,
}

/// SQLite row for note versions.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NoteVersionRow {
    pub id: String,
    pub note_id: String,
    pub body: String,
    pub created_at: String,
}

/// SQLite row for note tags.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NoteTagRow {
    pub note_id: String,
    pub tag: String,
}

/// SQLite row for note links.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NoteLinkRow {
    pub source_id: String,
    pub target_id: String,
}

// ── Row → Domain conversions ────────────────────────────────────────────

impl From<NotebookRow> for Notebook {
    fn from(r: NotebookRow) -> Self {
        Self {
            id: r.id,
            parent_id: r.parent_id,
            title: r.title,
            icon: r.icon,
            sort_order: r.sort_order,
            created_at: r.created_at.parse().unwrap_or_default(),
            updated_at: r.updated_at.parse().unwrap_or_default(),
        }
    }
}

impl From<NoteRow> for Note {
    fn from(r: NoteRow) -> Self {
        Self {
            id: r.id,
            notebook_id: r.notebook_id,
            title: r.title,
            body: r.body,
            body_html: r.body_html,
            pinned: r.pinned != 0,
            archived: r.archived != 0,
            tags: vec![], // populated separately
            created_at: r.created_at.parse().unwrap_or_default(),
            updated_at: r.updated_at.parse().unwrap_or_default(),
        }
    }
}

impl From<NoteVersionRow> for NoteVersion {
    fn from(r: NoteVersionRow) -> Self {
        Self {
            id: r.id,
            note_id: r.note_id,
            body: r.body,
            created_at: r.created_at.parse().unwrap_or_default(),
        }
    }
}
```

**Step 3: Run tests**

Run: `cargo nextest run -p feature-notes`
Expected: `test_migration_sql_not_empty` PASSES

**Step 4: Commit**

```bash
git add crates/feature-notes/migrations/ crates/feature-notes/src/models.rs
git commit -m "feat(notes): add migration SQL and domain models"
```

---

### Task 3: NoteRepo — CRUD operations

**Files:**
- Modify: `crates/feature-notes/src/repo.rs`

**Step 1: Write failing tests**

Add tests at the bottom of `repo.rs` in a `#[cfg(test)] mod tests` block. Tests should cover: create note, get note, list notes, update note, delete note, create notebook, list notebooks.

Each test should use `StoragePool::connect_in_memory()` and run the migration SQL before testing.

**Step 2: Implement NoteRepo**

```rust
use sqlx::SqlitePool;
use crate::models::*;

#[derive(Debug, Clone)]
pub struct NoteRepo {
    pool: SqlitePool,
}

impl NoteRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // ── Notes ────────────────────────────────────────

    pub async fn create_note(&self, row: &NoteRow) -> Result<NoteRow, storage::StorageError> {
        // INSERT INTO notes ... RETURNING *
    }

    pub async fn get_note(&self, id: &str) -> Result<Option<NoteRow>, storage::StorageError> {
        // SELECT * FROM notes WHERE id = ?
    }

    pub async fn list_notes(&self, notebook_id: Option<&str>) -> Result<Vec<NoteRow>, storage::StorageError> {
        // SELECT * FROM notes WHERE (notebook_id = ? OR ? IS NULL) AND archived = 0
        // ORDER BY pinned DESC, updated_at DESC
    }

    pub async fn update_note(&self, id: &str, title: Option<&str>, body: Option<&str>, body_html: Option<&str>, pinned: Option<bool>) -> Result<NoteRow, storage::StorageError> {
        // Dynamic UPDATE with RETURNING *
    }

    pub async fn delete_note(&self, id: &str) -> Result<bool, storage::StorageError> {
        // DELETE FROM notes WHERE id = ?
    }

    pub async fn count(&self) -> Result<i64, storage::StorageError> {
        // SELECT COUNT(*) FROM notes
    }

    pub async fn search_notes(&self, query: &str) -> Result<Vec<NoteRow>, storage::StorageError> {
        // SELECT * FROM notes WHERE title LIKE ? OR body LIKE ?
    }

    // ── Tags ─────────────────────────────────────────

    pub async fn get_tags(&self, note_id: &str) -> Result<Vec<String>, storage::StorageError> {
        // SELECT tag FROM note_tags WHERE note_id = ?
    }

    pub async fn set_tags(&self, note_id: &str, tags: &[String]) -> Result<(), storage::StorageError> {
        // DELETE all, INSERT new (in transaction)
    }

    // ── Notebooks ────────────────────────────────────

    pub async fn create_notebook(&self, row: &NotebookRow) -> Result<NotebookRow, storage::StorageError> {
        // INSERT INTO notebooks ... RETURNING *
    }

    pub async fn list_notebooks(&self) -> Result<Vec<NotebookRow>, storage::StorageError> {
        // SELECT * FROM notebooks ORDER BY sort_order, title
    }

    pub async fn update_notebook(&self, id: &str, title: Option<&str>, icon: Option<&str>, parent_id: Option<Option<&str>>) -> Result<NotebookRow, storage::StorageError> {
        // Dynamic UPDATE with RETURNING *
    }

    pub async fn delete_notebook(&self, id: &str) -> Result<bool, storage::StorageError> {
        // DELETE FROM notebooks WHERE id = ?
    }

    // ── Links ────────────────────────────────────────

    pub async fn set_links(&self, source_id: &str, target_ids: &[String]) -> Result<(), storage::StorageError> {
        // DELETE existing links for source, INSERT new ones
    }

    pub async fn get_links_from(&self, source_id: &str) -> Result<Vec<NoteLinkRow>, storage::StorageError> {
        // SELECT * FROM note_links WHERE source_id = ?
    }

    pub async fn get_links_to(&self, target_id: &str) -> Result<Vec<NoteLinkRow>, storage::StorageError> {
        // SELECT * FROM note_links WHERE target_id = ?
    }

    pub async fn get_all_links(&self) -> Result<Vec<NoteLinkRow>, storage::StorageError> {
        // SELECT * FROM note_links (for graph view)
    }

    // ── Versions ─────────────────────────────────────

    pub async fn create_version(&self, row: &NoteVersionRow) -> Result<NoteVersionRow, storage::StorageError> {
        // INSERT INTO note_versions ... RETURNING *
    }

    pub async fn list_versions(&self, note_id: &str) -> Result<Vec<NoteVersionRow>, storage::StorageError> {
        // SELECT * FROM note_versions WHERE note_id = ? ORDER BY created_at DESC
    }

    pub async fn prune_versions(&self, note_id: &str, max_versions: i64) -> Result<u64, storage::StorageError> {
        // DELETE oldest versions beyond max
    }
}
```

**Step 3: Run tests**

Run: `cargo nextest run -p feature-notes`
Expected: All PASS

**Step 4: Commit**

```bash
git add crates/feature-notes/src/repo.rs
git commit -m "feat(notes): implement NoteRepo with CRUD, tags, links, versions"
```

---

### Task 4: Desktop IPC — shared types + commands

**Files:**
- Modify: `crates/desktop-shared/src/commands.rs` (add Note/Notebook response/param types)
- Create: `crates/desktop/src/commands/notes.rs` (Tauri command handlers)
- Modify: `crates/desktop/src/commands/mod.rs` (register module)
- Modify: `crates/desktop/src/main.rs` (register commands in generate_handler!)
- Modify: `crates/dev-api/src/main.rs` (add note_* dispatch routes)

**Step 1: Add shared types to desktop-shared**

Add to `crates/desktop-shared/src/commands.rs`:

```rust
// ── Notes ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteResponse {
    pub id: String,
    pub notebook_id: Option<String>,
    pub title: String,
    pub body: String,
    pub body_html: Option<String>,
    pub pinned: bool,
    pub archived: bool,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteCreateParams {
    pub title: String,
    pub notebook_id: Option<String>,
    pub body: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteUpdateParams {
    pub id: String,
    pub title: Option<String>,
    pub body: Option<String>,
    pub body_html: Option<String>,
    pub pinned: Option<bool>,
    pub notebook_id: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookResponse {
    pub id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub icon: Option<String>,
    pub sort_order: i32,
    pub note_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookCreateParams {
    pub title: String,
    pub parent_id: Option<String>,
    pub icon: Option<String>,
}
```

**Step 2: Create Tauri command handlers**

Create `crates/desktop/src/commands/notes.rs` with commands:
- `note_list(state, notebook_id: Option<String>) -> Vec<NoteResponse>`
- `note_create(state, app, params: NoteCreateParams) -> NoteResponse`
- `note_update(state, app, params: NoteUpdateParams) -> NoteResponse`
- `note_delete(state, app, id: String) -> bool`
- `note_get(state, id: String) -> NoteResponse`
- `note_search(state, query: String) -> Vec<NoteResponse>`
- `notebook_list(state) -> Vec<NotebookResponse>`
- `notebook_create(state, app, params: NotebookCreateParams) -> NotebookResponse`
- `notebook_update(state, app, id: String, title: Option<String>, icon: Option<String>) -> NotebookResponse`
- `notebook_delete(state, app, id: String) -> bool`

Follow the same pattern as `tasks.rs`: extract params, call repo, emit `entity:updated`, return response.

**Step 3: Register in main.rs**

Add all `note_*` and `notebook_*` commands to `generate_handler![]` in `crates/desktop/src/main.rs`.

**Step 4: Add dev-api routes**

Add dispatch entries in `crates/dev-api/src/main.rs` for each note command.

**Step 5: Build and verify**

Run: `cargo build -p desktop` and `cargo build -p dev-api`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/desktop-shared/src/commands.rs crates/desktop/src/commands/ crates/dev-api/src/main.rs
git commit -m "feat(notes): add IPC commands for notes and notebooks"
```

---

### Task 5: Frontend — types + hooks + route

**Files:**
- Modify: `desktop-ui/src/lib/types.ts` (add Note, Notebook types)
- Modify: `desktop-ui/src/App.tsx` (add /notes route)
- Modify: `desktop-ui/src/components/layout/Sidebar.tsx` (add Notes entry)
- Create: `desktop-ui/src/components/notes/NotesView.tsx` (placeholder)

**Step 1: Add TypeScript types**

Add to `desktop-ui/src/lib/types.ts`:

```typescript
// ── Notes ────────────────────────────────────────
export interface Note {
  id: string;
  notebookId: string | null;
  title: string;
  body: string;
  bodyHtml: string | null;
  pinned: boolean;
  archived: boolean;
  tags: string[];
  createdAt: string;
  updatedAt: string;
}

export interface NoteCreateParams {
  title: string;
  notebookId?: string;
  body?: string;
  tags?: string[];
}

export interface NoteUpdateParams {
  id: string;
  title?: string;
  body?: string;
  bodyHtml?: string;
  pinned?: boolean;
  notebookId?: string;
  tags?: string[];
}

export interface Notebook {
  id: string;
  parentId: string | null;
  title: string;
  icon: string | null;
  sortOrder: number;
  noteCount: number;
}

export interface NotebookCreateParams {
  title: string;
  parentId?: string;
  icon?: string;
}
```

**Step 2: Add route**

In `desktop-ui/src/App.tsx`, add lazy import and route:

```tsx
const NotesView = lazy(() => import("./components/notes/NotesView"));

// In router config:
{ path: "/notes", element: <NotesView /> },
```

**Step 3: Add sidebar entry**

In `Sidebar.tsx`, add to the items array:

```tsx
{ key: "Notes", icon: FileText, path: "/notes" },
```

Import `FileText` from `lucide-react`.

**Step 4: Create placeholder NotesView**

Create `desktop-ui/src/components/notes/NotesView.tsx`:

```tsx
import { useQuery } from "../../hooks/useQuery";
import type { Note, Notebook } from "../../lib/types";

export default function NotesView() {
  const { data: notebooks } = useQuery<Notebook[]>("notebook_list", undefined, []);
  const { data: notes } = useQuery<Note[]>("note_list", undefined, []);

  return (
    <div className="h-full flex gap-2">
      <div className="w-56 glass-panel rounded-2xl p-3">
        <h2 className="text-sm font-medium text-muted mb-2">Notebooks</h2>
        {notebooks.map((nb) => (
          <div key={nb.id} className="text-sm text-secondary py-1 px-2 rounded-lg hover:bg-surface-raised cursor-pointer">
            {nb.icon && <span className="mr-1.5">{nb.icon}</span>}
            {nb.title}
          </div>
        ))}
      </div>
      <div className="w-72 glass-panel rounded-2xl p-3">
        <h2 className="text-sm font-medium text-muted mb-2">Notes ({notes.length})</h2>
        {notes.map((note) => (
          <div key={note.id} className="p-2 rounded-xl hover:bg-surface-raised cursor-pointer">
            <div className="text-sm font-medium text-primary">{note.title}</div>
            <div className="text-xs text-muted truncate">{note.body.slice(0, 80)}</div>
          </div>
        ))}
      </div>
      <div className="flex-1 glass-panel rounded-2xl p-6">
        <div className="text-muted text-sm">Select a note to edit</div>
      </div>
    </div>
  );
}
```

**Step 5: Verify**

Run: `cd desktop-ui && bun run build`
Expected: PASS

**Step 6: Commit**

```bash
git add desktop-ui/src/
git commit -m "feat(notes): add Notes route, types, and placeholder view"
```

---

### Task 6: Frontend — NoteList + NoteCard components

**Files:**
- Create: `desktop-ui/src/components/notes/NoteList.tsx`
- Create: `desktop-ui/src/components/notes/NoteCard.tsx`
- Modify: `desktop-ui/src/components/notes/NotesView.tsx`

**Step 1: Build NoteCard**

A compact card showing title, snippet, tags, and timestamp. Uses `bg-surface-base` with `hover:bg-surface-raised`. Pinned notes show a pin icon. Selected note gets `ring-1 ring-brand/30`.

**Step 2: Build NoteList**

Receives `notes: Note[]`, `selectedId`, `onSelect`, `onCreate`. Includes a search input at top (`glass-input`), a "New Note" button, and maps notes to NoteCards. Sorts: pinned first, then by updated_at descending.

**Step 3: Integrate into NotesView**

Replace inline note list with `<NoteList>` component. Track `selectedNoteId` state. Pass it down and show selected note content in the editor panel.

**Step 4: Verify**

Run: `cd desktop-ui && bun run build`

**Step 5: Commit**

```bash
git add desktop-ui/src/components/notes/
git commit -m "feat(notes): add NoteList and NoteCard components"
```

---

### Task 7: Frontend — NotebookTree component

**Files:**
- Create: `desktop-ui/src/components/notes/NotebookTree.tsx`
- Modify: `desktop-ui/src/components/notes/NotesView.tsx`

**Step 1: Build NotebookTree**

A recursive tree component. Props: `notebooks: Notebook[]`, `selectedId`, `onSelect`, `onCreate`. Renders notebooks as collapsible tree nodes with emoji icons. "All Notes" entry at top (selectedId = null). "New Notebook" button at bottom. Uses `text-sm` with indentation per depth level.

**Step 2: Integrate into NotesView**

Replace inline notebook list with `<NotebookTree>`. Track `selectedNotebookId` state, filter notes by notebook when selected.

**Step 3: Verify and commit**

```bash
git add desktop-ui/src/components/notes/
git commit -m "feat(notes): add NotebookTree with recursive hierarchy"
```

---

### Task 8: Frontend — TipTap editor setup (port from HelixNotes)

**Files:**
- Create: `desktop-ui/src/components/notes/editor/EditorCore.tsx`
- Create: `desktop-ui/src/components/notes/editor/EditorToolbar.tsx`
- Create: `desktop-ui/src/components/notes/NoteEditor.tsx`
- Modify: `desktop-ui/package.json` (add TipTap deps)

**Step 1: Install TipTap packages**

```bash
cd desktop-ui && bun add @tiptap/react @tiptap/starter-kit @tiptap/pm \
  @tiptap/extension-code-block-lowlight @tiptap/extension-color \
  @tiptap/extension-highlight @tiptap/extension-image @tiptap/extension-link \
  @tiptap/extension-placeholder @tiptap/extension-subscript \
  @tiptap/extension-superscript @tiptap/extension-table \
  @tiptap/extension-table-cell @tiptap/extension-table-header \
  @tiptap/extension-table-row @tiptap/extension-task-item \
  @tiptap/extension-task-list @tiptap/extension-text-align \
  @tiptap/extension-text-style @tiptap/extension-typography \
  @tiptap/extension-underline lowlight katex
```

**Step 2: Build EditorCore**

Port the TipTap extension config from HelixNotes `Editor.svelte` lines 2116-2154. Use `@tiptap/react`'s `useEditor` hook + `<EditorContent>` component. Configure all extensions (StarterKit, Table, TaskList, Link, Image, CodeBlockLowlight, Highlight, Typography, etc.).

Reference: `../helixnotes/src/lib/components/Editor.svelte` for exact extension config.

Key difference: Use React's `useEditor()` hook instead of Svelte's `new Editor()`.

```tsx
import { useEditor, EditorContent } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
// ... other imports

export function EditorCore({ content, onUpdate }: EditorCoreProps) {
  const editor = useEditor({
    extensions: [
      StarterKit.configure({ codeBlock: false }),
      // ... all extensions from HelixNotes config
    ],
    content,
    onUpdate: ({ editor }) => {
      onUpdate(editor.getHTML(), editor.storage.markdown?.getMarkdown?.());
    },
  });

  return <EditorContent editor={editor} className="editor-content prose" />;
}
```

**Step 3: Build EditorToolbar**

A horizontal toolbar with formatting buttons. Uses `glass-button` tokens. Groups: text formatting (bold/italic/underline/strike), headings (H1-H3), lists (bullet/ordered/task), insert (table/code/image/link), alignment.

Each button calls `editor.chain().focus().toggleBold().run()` etc.

Reference: HelixNotes toolbar pattern (same TipTap chain API).

**Step 4: Build NoteEditor**

Orchestrates EditorToolbar + EditorCore. Props: `note: Note`, `onSave`. Implements debounced auto-save (1s after last change). Shows title input at top (contentEditable or input field).

**Step 5: Integrate into NotesView**

When a note is selected, show `<NoteEditor>` in the right panel. Wire up save mutation.

**Step 6: Add editor styles**

Add TipTap prose styles to `desktop-ui/src/styles/editor.css`. Import in `index.css`. Style code blocks, tables, task lists, links, images, placeholders to match glass design.

**Step 7: Verify**

Run: `cd desktop-ui && bun run build`

**Step 8: Commit**

```bash
git add desktop-ui/
git commit -m "feat(notes): add TipTap rich text editor with toolbar"
```

---

### Task 9: Frontend — Slash commands

**Files:**
- Create: `desktop-ui/src/components/notes/editor/SlashCommandMenu.tsx`
- Modify: `desktop-ui/src/components/notes/editor/EditorCore.tsx`

**Step 1: Port SlashCommands extension**

Port the slash command ProseMirror plugin from HelixNotes `Editor.svelte` lines 650-741. This is framework-agnostic ProseMirror code — extract it into a standalone TipTap extension.

**Step 2: Build SlashCommandMenu React component**

A floating popup that appears on `/` keystroke. Shows filterable list of block types (Heading, List, Code Block, Table, etc.). Keyboard navigable (arrow keys + enter). Uses `glass-panel` styling.

**Step 3: Wire into EditorCore**

Register the SlashCommands extension and render `<SlashCommandMenu>` as a portal positioned relative to cursor.

**Step 4: Verify and commit**

```bash
git add desktop-ui/src/components/notes/editor/
git commit -m "feat(notes): add slash command menu"
```

---

### Task 10: Frontend — CRUD integration (create, delete, pin)

**Files:**
- Modify: `desktop-ui/src/components/notes/NotesView.tsx`
- Modify: `desktop-ui/src/components/notes/NoteList.tsx`

**Step 1: Wire up create**

"New Note" button calls `useMutation("note_create")` with title "Untitled" and current notebook_id. On success, select the new note and focus the editor.

**Step 2: Wire up delete**

Right-click context menu or delete button on NoteCard. Confirmation dialog. Calls `useMutation("note_delete")`. On success, clear selection if deleted note was selected.

**Step 3: Wire up pin/unpin**

Toggle button on NoteCard. Calls `useMutation("note_update", "params")` with `{ id, pinned: !current }`.

**Step 4: Wire up event refresh**

`useEvent("entity:updated", ...)` to refetch notes/notebooks when backend emits changes.

**Step 5: Verify end-to-end**

Start dev server (`cargo run -p dev-api` + `cd desktop-ui && bun run dev`). Create notes, edit, pin, delete. Verify data persists.

**Step 6: Commit**

```bash
git add desktop-ui/src/components/notes/
git commit -m "feat(notes): wire up CRUD operations end-to-end"
```

---

## Phase 2: Knowledge (Wiki-links, Graph, Search, Tags)

### Task 11: Custom TipTap nodes — WikiLink + MathBlock

**Files:**
- Create: `desktop-ui/src/components/notes/editor/WikiLinkNode.tsx`
- Create: `desktop-ui/src/components/notes/editor/MathNode.tsx`
- Modify: `desktop-ui/src/components/notes/editor/EditorCore.tsx`

Port WikiLink mark and MathBlock/MathInline nodes from HelixNotes `Editor.svelte` lines 802-950 (WikiLink) and 331-390 (Math). These are ProseMirror node/mark definitions — framework-agnostic. Wrap in React rendering for node views.

WikiLink autocomplete: `[[` trigger opens a search popup (fuzzy match against note titles via `useQuery("note_search")`). On select, insert WikiLinkNode with target note ID.

---

### Task 12: Backend — link extraction on save

**Files:**
- Create: `crates/feature-notes/src/link_parser.rs`
- Modify: `crates/desktop/src/commands/notes.rs`

On `note_update`, parse the markdown body for `[[Note Title]]` patterns. Resolve titles to note IDs. Call `repo.set_links(note_id, target_ids)`. Also parse `@task:id` / `@project:id` patterns and update `note_entity_mentions`.

---

### Task 13: Graph view

**Files:**
- Create: `desktop-ui/src/components/notes/GraphView.tsx`
- Modify: `desktop-ui/src/components/notes/NotesView.tsx`

Build a force-directed graph using `d3-force` (add dep). Nodes = notes, edges = wiki-links. Fetch all links via `useQuery("note_links_all")`. Active note highlighted in brand orange. Click node to navigate. Toggle between editor and graph view.

---

### Task 14: Tags UI

**Files:**
- Create: `desktop-ui/src/components/notes/NoteTags.tsx`
- Modify: `desktop-ui/src/components/notes/NoteEditor.tsx`
- Modify: `desktop-ui/src/components/notes/NoteList.tsx`

Tag pills below editor title. Add/remove with inline input. Filter note list by tag (tag cloud in sidebar or filter chips above list).

---

### Task 15: Full-text search (backend + frontend)

**Files:**
- Modify: `crates/feature-notes/src/repo.rs` (enhance search_notes)
- Create: `desktop-ui/src/components/notes/NoteSearchBar.tsx`
- Modify: `desktop-ui/src/components/notes/NotesView.tsx`

Backend: Improve `search_notes` to search across title, body, and tags with relevance ranking. Frontend: Debounced search input that calls `note_search` and displays results replacing the note list.

---

## Phase 3: Integration (Entity mentions, Version history, Cross-linking)

### Task 16: Entity mentions (@task, @project)

**Files:**
- Create: `desktop-ui/src/components/notes/editor/EntityMention.tsx`
- Modify: `desktop-ui/src/components/notes/editor/EditorCore.tsx`

Custom TipTap node for `@` mentions. `@` trigger opens entity search popup with type tabs. Renders as colored chip (orange for tasks, blue for projects). Clicking navigates to entity detail page.

---

### Task 17: Cross-feature linked notes

**Files:**
- Modify: `desktop-ui/src/components/views/TaskDetail.tsx` (or equivalent)
- Modify: `desktop-ui/src/components/views/ProjectDetail.tsx`
- Add: IPC command `note_list_by_entity(entity_type, entity_id)`

Show "Linked Notes" section on task and project detail pages. Query `note_entity_mentions` table for notes referencing the entity. Display as compact list with click-to-navigate.

---

### Task 18: Version history

**Files:**
- Create: `desktop-ui/src/components/notes/NoteVersionHistory.tsx`
- Modify: `desktop-ui/src/components/notes/NoteEditor.tsx`
- Add: IPC commands `note_version_list`, `note_version_restore`
- Modify: `crates/desktop/src/commands/notes.rs`

Backend: On save, check if last version > 5 min ago, if so create snapshot. Prune beyond max (50). Frontend: Side panel toggled from editor toolbar showing version list with timestamps. Click to preview, button to restore.

---

### Task 19: Image paste from clipboard

**Files:**
- Modify: `desktop-ui/src/components/notes/editor/EditorCore.tsx`
- Add: IPC command `note_save_attachment`
- Modify: `crates/desktop/src/commands/notes.rs`

Handle paste event in TipTap editor. If clipboard contains image, save to data dir (via IPC command), insert Image node with local path. Reference HelixNotes' `handleFilePaste` pattern.

---

### Task 20: Keyboard shortcuts + polish

**Files:**
- Modify: `desktop-ui/src/components/notes/NotesView.tsx`
- Modify: `desktop-ui/src/components/notes/NoteEditor.tsx`

Add keyboard shortcuts: Cmd+N (new note), Cmd+S (force save), Cmd+Backspace (delete), Cmd+Shift+F (search). Polish transitions, loading states, empty states. Ensure glass design consistency.

---

## Verification Checklist

After each phase, verify:

- [ ] `cargo build --workspace` passes
- [ ] `cargo nextest run -p feature-notes` all tests pass
- [ ] `cargo clippy --workspace --all-targets --all-features` — 0 warnings
- [ ] `cd desktop-ui && bun run build` passes
- [ ] `cd desktop-ui && bun run lint:fix` — clean
- [ ] Manual test: create note, edit, save, reload, verify persistence
